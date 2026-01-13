//! Agent loop with tool calling support
//!
//! The agent operates in two loops:
//! - Outer loop: User conversation (user input → agent response)
//! - Inner loop: Tool execution (agent requests tool → execute → continue)

use crate::cli::Console;
use crate::context::ContextManager;
use crate::conversation::Conversation;
use crate::debugger::Debugger;
use crate::llm::{
    AnthropicProvider, ContentBlock, Message, MessageRequest, MessageResponse, StopReason,
    ThinkingConfig, ToolChoice, ToolDefinition,
};
use crate::permissions::{PermissionDecision, PermissionManager, PermissionRequest};
use crate::tools::ToolRegistry;
use anyhow::Result;

/// Maximum number of tool calls in a single turn
const MAX_TOOL_ITERATIONS: usize = 50;

/// Main agent that orchestrates the conversation loop
pub struct Agent {
    console: Console,
    llm_provider: AnthropicProvider,
    conversation: Conversation,
    tool_registry: ToolRegistry,
    permission_manager: PermissionManager,
    context_manager: ContextManager,
    debugger: Debugger,
}

impl Agent {
    /// Create a new Agent with all components
    pub fn new(
        console: Console,
        llm_provider: AnthropicProvider,
        tool_registry: ToolRegistry,
        context_manager: ContextManager,
        debugger: Debugger,
    ) -> Result<Self> {
        tracing::info!("Creating new Agent");

        let conversation = Conversation::new()?;
        tracing::info!("Conversation initialized: {}", conversation.id());

        if debugger.is_enabled() {
            tracing::info!("Debugger enabled: {:?}", debugger.session_dir());
        }

        Ok(Self {
            console,
            llm_provider,
            conversation,
            tool_registry,
            permission_manager: PermissionManager::new(),
            context_manager,
            debugger,
        })
    }

    /// Get the conversation ID
    pub fn conversation_id(&self) -> &str {
        self.conversation.id()
    }

    /// Get a reference to the console
    pub fn console(&self) -> &Console {
        &self.console
    }

    /// Run the main agent loop
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Starting agent loop");
        self.console.print_banner();

        loop {
            // Outer loop: Read user input
            let user_input = match self.console.read_input() {
                Ok(input) => {
                    tracing::debug!("User input received: {}", input);
                    input
                }
                Err(e) => {
                    tracing::error!("Failed to read user input: {}", e);
                    self.console
                        .print_error(&format!("Failed to read input: {}", e));
                    continue;
                }
            };

            // Check for exit commands
            if user_input.to_lowercase() == "exit" || user_input.to_lowercase() == "quit" {
                tracing::info!("User requested exit");
                self.console.print_system("Goodbye!");
                break;
            }

            // Skip empty input
            if user_input.trim().is_empty() {
                tracing::debug!("Empty input, skipping");
                continue;
            }

            // Print separator for readability
            self.console.println();

            // Process the message (may involve multiple tool calls)
            tracing::info!("Processing user message");
            if let Err(e) = self.process_turn(&user_input).await {
                tracing::error!("Error processing message: {:?}", e);
                self.console
                    .print_error(&format!("Error processing message: {}", e));
            }

            // Print separator after response
            self.console.println();
            self.console.print_separator();
        }

        tracing::info!("Agent loop ended");
        Ok(())
    }

    /// Process a single user turn (may involve multiple tool calls)
    async fn process_turn(&mut self, user_message: &str) -> Result<()> {
        tracing::debug!("Processing turn: {}", user_message);

        // Get current conversation messages
        let history = self.conversation.get_messages()?;
        let mut messages: Vec<Message> = history;

        // Add the user message
        let user_msg = Message::user(user_message);
        messages.push(user_msg.clone());

        // Save the user message immediately
        self.conversation.add_message_raw(&user_msg)?;

        // Get tool definitions
        let tools = self.tool_registry.get_definitions();

        // Build system prompt with context
        let system_prompt = self
            .context_manager
            .build_full_system_prompt(&messages)
            .await?;

        // Inner loop: Process tool calls until done
        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > MAX_TOOL_ITERATIONS {
                tracing::warn!("Maximum tool iterations reached");
                self.console
                    .print_system("Maximum tool iterations reached. Stopping.");
                break;
            }

            // Build and log the request
            // Use a larger thinking budget to encourage more thorough reasoning
            let thinking_config = Some(ThinkingConfig::enabled(16000));
            self.log_api_request(&messages, &system_prompt, &tools, &thinking_config)?;

            // Call the LLM with extended thinking enabled
            let response = self
                .llm_provider
                .send_with_tools(
                    messages.clone(),
                    Some(&system_prompt),
                    tools.clone(),
                    Some(ToolChoice::auto()),
                    thinking_config,
                )
                .await?;

            // Log the response
            self.debugger.log_api_response(&response)?;

            // Process the response
            let (should_continue, new_messages) = self.process_response(&response).await?;

            // Add new messages to the conversation and save incrementally
            for msg in &new_messages {
                messages.push(msg.clone());
                self.conversation.add_message_raw(msg)?;
            }

            if !should_continue {
                // Agent is done with this turn
                break;
            }
        }

        Ok(())
    }

    /// Log an API request to the debugger
    fn log_api_request(
        &self,
        messages: &[Message],
        system_prompt: &str,
        tools: &[ToolDefinition],
        thinking: &Option<ThinkingConfig>,
    ) -> Result<()> {
        let request = MessageRequest {
            model: self.llm_provider.model().to_string(),
            max_tokens: self.llm_provider.max_tokens(),
            messages: messages.to_vec(),
            system: Some(system_prompt.to_string()),
            tools: if tools.is_empty() {
                None
            } else {
                Some(tools.to_vec())
            },
            tool_choice: Some(ToolChoice::auto()),
            thinking: thinking.clone(),
            temperature: None,
            stream: None,
        };

        self.debugger.log_api_request(&request)
    }

    /// Process a response from the LLM
    ///
    /// Returns (should_continue, new_messages_to_add)
    async fn process_response(
        &mut self,
        response: &MessageResponse,
    ) -> Result<(bool, Vec<Message>)> {
        let mut new_messages = Vec::new();
        let mut tool_results: Vec<ContentBlock> = Vec::new();
        let mut has_tool_use = false;

        // Process each content block
        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    // Print text to the user
                    self.console.print_assistant(text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    has_tool_use = true;
                    tracing::info!("Tool use requested: {} ({})", name, id);

                    // Log the tool call to debugger
                    let _ = self.debugger.log_tool_call(id, name, input);

                    // Get tool info for permission prompt
                    let tool_info = self.tool_registry.get_tool_info(name, input);

                    // Check if we need permission
                    let should_execute = if self.tool_registry.requires_permission(name) {
                        // Check auto-decision first
                        match self.permission_manager.check_auto_decision(name) {
                            Some(true) => {
                                // Auto-allowed
                                if let Some(info) = &tool_info {
                                    self.console
                                        .print_tool_action(name, &info.action_description);
                                }
                                true
                            }
                            Some(false) => {
                                // Auto-denied
                                self.console.print_system(&format!(
                                    "Tool {} is blocked (always deny)",
                                    name
                                ));
                                false
                            }
                            None => {
                                // Ask user
                                let request = if let Some(info) = tool_info.clone() {
                                    PermissionRequest {
                                        tool_name: info.name,
                                        action_description: info.action_description,
                                        details: info.details,
                                    }
                                } else {
                                    PermissionRequest::new(name, format!("Execute tool: {}", name))
                                };

                                let decision = self.console.ask_permission(&request)?;
                                self.permission_manager.process_decision(name, decision);

                                matches!(
                                    decision,
                                    PermissionDecision::Allow | PermissionDecision::AlwaysAllow
                                )
                            }
                        }
                    } else {
                        // Tool doesn't require permission
                        true
                    };

                    // Execute the tool
                    let result = if should_execute {
                        match self.tool_registry.execute(name, input).await {
                            Ok(result) => {
                                self.console.print_tool_result(&result.output, result.is_error);
                                // Log tool result to debugger
                                let _ = self.debugger.log_tool_result(
                                    id,
                                    name,
                                    &result.output,
                                    result.is_error,
                                );
                                ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: Some(result.output),
                                    is_error: if result.is_error { Some(true) } else { None },
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Tool execution failed: {}", e);
                                self.console.print_tool_result(&error_msg, true);
                                // Log tool error to debugger
                                let _ =
                                    self.debugger
                                        .log_tool_result(id, name, &error_msg, true);
                                ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: Some(error_msg),
                                    is_error: Some(true),
                                }
                            }
                        }
                    } else {
                        // Permission denied
                        let denied_msg = "Permission denied by user";
                        let _ = self.debugger.log_tool_result(id, name, denied_msg, true);
                        ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: Some(denied_msg.to_string()),
                            is_error: Some(true),
                        }
                    };

                    tool_results.push(result);
                }
                ContentBlock::Thinking { thinking, .. } => {
                    // Display thinking to the user
                    self.console.print_thinking_block(thinking);
                    tracing::debug!("Agent thinking: {}", thinking);
                }
                ContentBlock::RedactedThinking { .. } => {
                    // Ignore redacted thinking
                }
                ContentBlock::ToolResult { .. } => {
                    // Tool results in assistant messages shouldn't happen,
                    // but ignore if they do
                    tracing::warn!("Unexpected ToolResult in assistant response");
                }
            }
        }

        // If there were tool uses, add the assistant message with tool uses and user message with results
        if has_tool_use {
            // Add the assistant message (with tool uses)
            new_messages.push(Message::assistant_with_blocks(response.content.clone()));

            // Add the user message with tool results
            if !tool_results.is_empty() {
                new_messages.push(Message::user_with_blocks(tool_results));
            }
        }

        // Determine if we should continue
        let should_continue = matches!(response.stop_reason, Some(StopReason::ToolUse));

        Ok((should_continue, new_messages))
    }
}
