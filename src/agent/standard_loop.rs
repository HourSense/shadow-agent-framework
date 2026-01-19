//! Standard Agent Loop
//!
//! The main agent implementation that handles:
//! - Input → LLM → Tools → Output cycle
//! - Context injection before LLM calls
//! - Session persistence
//! - Debug logging (when enabled)
//! - Streaming responses (when enabled)
//! - Automatic conversation naming (after first turn)

use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;
use serde_json::Value;

use crate::core::{FrameworkResult, InputMessage};
use crate::helpers::{ConversationNamer, Debugger};
use crate::hooks::HookContext;
use crate::llm::{
    AnthropicProvider, ContentBlock, ContentBlockStart, ContentDelta, Message, StopReason,
    StreamEvent,
};
use crate::runtime::AgentInternals;
use crate::tools::ToolResult;

use super::config::AgentConfig;
use super::executor::ToolExecutor;

/// Standard agent that handles the full agent loop
///
/// # Example
///
/// ```ignore
/// let config = AgentConfig::new("You are helpful")
///     .with_tools(tools);
///
/// let agent = StandardAgent::new(config, llm);
///
/// let handle = runtime.spawn(session, |internals| {
///     agent.run(internals)
/// }).await;
/// ```
pub struct StandardAgent {
    config: AgentConfig,
    llm: Arc<AnthropicProvider>,
}

impl StandardAgent {
    /// Create a new standard agent
    pub fn new(config: AgentConfig, llm: Arc<AnthropicProvider>) -> Self {
        Self { config, llm }
    }

    /// Run the agent loop
    ///
    /// This is the main entry point - pass this to `runtime.spawn()`.
    pub async fn run(self, mut internals: AgentInternals) -> FrameworkResult<()> {
        tracing::info!("[StandardAgent] Started, waiting for input...");

        // Initialize debugger if enabled
        if self.config.debug_enabled {
            let session_dir = internals
                .session
                .storage()
                .session_dir(internals.session.session_id());

            match Debugger::new(&session_dir) {
                Ok(debugger) => {
                    tracing::info!(
                        "[StandardAgent] Debug logging enabled at {:?}",
                        debugger.dir()
                    );
                    internals.context.insert_resource(debugger);
                }
                Err(e) => {
                    tracing::warn!("[StandardAgent] Failed to initialize debugger: {}", e);
                }
            }
        }

        loop {
            // Signal we're ready for input
            internals.set_idle().await;

            // Wait for next message
            match internals.receive().await {
                Some(InputMessage::UserInput(text)) => {
                    tracing::info!("[StandardAgent] Received: {}", text);
                    internals.set_processing().await;

                    // Run UserPromptSubmit hooks
                    let mut current_text = text.clone();
                    let mut should_process = true;

                    if let Some(ref hooks) = self.config.hooks {
                        let mut ctx = HookContext::user_prompt_submit(&mut internals, &text);
                        let result = hooks.run(&mut ctx);

                        // Hook may have modified the prompt
                        if let Some(modified) = ctx.user_prompt {
                            current_text = modified;
                        }

                        // Check if hook denied the prompt
                        if let Some(crate::hooks::PermissionDecision::Deny) = result.decision {
                            let reason = result.reason.unwrap_or_else(|| "Blocked by hook".to_string());
                            tracing::info!("[StandardAgent] UserPromptSubmit hook denied: {}", reason);
                            internals.send_error(format!("Prompt blocked: {}", reason));
                            should_process = false;
                        }
                    }

                    // Process the user message (if not blocked by hook)
                    if should_process {
                        if let Err(e) = self.process_turn(&mut internals, &current_text).await {
                            tracing::error!("[StandardAgent] Error processing turn: {}", e);
                            internals.send_error(format!("Error: {}", e));
                        }

                        // Auto-name conversation after first turn
                        if self.config.auto_name_conversation
                            && internals.context.current_turn == 0
                            && !internals.session.has_conversation_name()
                        {
                            self.generate_conversation_name(&mut internals).await;
                        }
                    }

                    // Signal turn complete
                    internals.send_done();

                    // Persist session if configured
                    if self.config.auto_save_session {
                        if let Err(e) = internals.session.save() {
                            tracing::error!("[StandardAgent] Failed to save session: {}", e);
                        }
                    }
                }

                Some(InputMessage::Interrupt) => {
                    tracing::info!("[StandardAgent] Interrupted");
                    internals.send_status("Interrupted");
                    internals.set_done().await;
                    break;
                }

                Some(InputMessage::Shutdown) | None => {
                    tracing::info!("[StandardAgent] Shutting down");
                    internals.set_done().await;
                    break;
                }

                _ => {
                    // Ignore other message types
                }
            }

            internals.next_turn();
        }

        Ok(())
    }

    /// Generate a conversation name using the ConversationNamer helper
    async fn generate_conversation_name(&self, internals: &mut AgentInternals) {
        tracing::debug!("[StandardAgent] Generating conversation name...");

        let namer = ConversationNamer::new(&self.llm);
        match namer.generate_name(internals.session.history()).await {
            Ok(name) => {
                tracing::info!("[StandardAgent] Generated conversation name: {}", name);
                if let Err(e) = internals.session.set_conversation_name(&name) {
                    tracing::warn!(
                        "[StandardAgent] Failed to save conversation name: {}",
                        e
                    );
                }
            }
            Err(e) => {
                tracing::warn!("[StandardAgent] Failed to generate conversation name: {}", e);
            }
        }
    }

    /// Process a single user turn (may involve multiple LLM calls for tool use)
    async fn process_turn(&self, internals: &mut AgentInternals, user_input: &str) -> Result<()> {
        // Add user message to history
        internals.session.add_message(Message::user(user_input))?;

        // Get tool definitions
        let tool_definitions = self.config.tool_definitions();

        let mut iterations = 0;

        // LLM loop - continues until no more tool calls
        loop {
            iterations += 1;
            if iterations > self.config.max_tool_iterations {
                tracing::warn!(
                    "[StandardAgent] Max tool iterations ({}) reached",
                    self.config.max_tool_iterations
                );
                internals.send_status("Max tool iterations reached");
                break;
            }

            // Get messages and apply context injections
            let mut messages = internals.session.history().to_vec();
            messages = self.config.injections.apply(internals, messages);

            tracing::info!(
                "[StandardAgent] Calling LLM with {} messages (iteration {})",
                messages.len(),
                iterations
            );

            // Log API request if debugger is enabled
            if let Some(debugger) = internals.context.get_resource::<Debugger>() {
                let tool_defs: Vec<serde_json::Value> = tool_definitions
                    .iter()
                    .map(|t| serde_json::to_value(t).unwrap_or_default())
                    .collect();

                if let Err(e) = debugger.log_api_request(
                    &messages,
                    Some(&self.config.system_prompt),
                    Some(&tool_defs),
                ) {
                    tracing::warn!("[StandardAgent] Failed to log API request: {}", e);
                }
            }

            // Choose streaming or non-streaming based on config
            let (content_blocks, stop_reason) = if self.config.streaming_enabled {
                self.call_llm_streaming(internals, messages, &tool_definitions)
                    .await?
            } else {
                self.call_llm_non_streaming(internals, messages, &tool_definitions)
                    .await?
            };

            tracing::info!(
                "[StandardAgent] LLM response: stop_reason={:?}",
                stop_reason
            );

            // Process tool use blocks and execute tools
            let mut tool_results: Vec<(String, ToolResult)> = Vec::new();

            for block in &content_blocks {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    tracing::info!("[StandardAgent] Tool use: {} ({})", name, id);

                    // Execute tool with permission check (if tools configured)
                    let result = if let Some(ref tools) = self.config.tools {
                        let hooks = self.config.hooks.as_deref();
                        ToolExecutor::execute_with_permission(internals, tools, hooks, name, id, input)
                            .await
                    } else {
                        ToolResult::error(format!(
                            "No tools configured, cannot execute: {}",
                            name
                        ))
                    };

                    tool_results.push((id.clone(), result));
                }
            }

            // Add assistant message to history
            internals
                .session
                .add_message(Message::assistant_with_blocks(content_blocks))?;

            // If there were tool calls, add results and continue loop
            if !tool_results.is_empty() {
                // Add tool results as a message
                let tool_result_blocks: Vec<ContentBlock> = tool_results
                    .into_iter()
                    .map(|(id, result)| {
                        ContentBlock::tool_result(&id, &result.output, result.is_error)
                    })
                    .collect();

                internals
                    .session
                    .add_message(Message::user_with_blocks(tool_result_blocks))?;

                // Continue to next LLM call
                continue;
            }

            // No tool calls - check if we should stop
            match stop_reason {
                Some(StopReason::EndTurn) | Some(StopReason::StopSequence) | None => {
                    // Done with this turn
                    break;
                }
                Some(StopReason::ToolUse) => {
                    // Shouldn't happen if tool_results is empty, but continue just in case
                    continue;
                }
                Some(StopReason::MaxTokens) => {
                    internals.send_status("Response truncated (max tokens)");
                    break;
                }
                Some(StopReason::PauseTurn) => {
                    // Model paused, wait for next input
                    break;
                }
                Some(StopReason::Refusal) => {
                    internals.send_status("Model refused to respond");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Call LLM without streaming (original behavior)
    async fn call_llm_non_streaming(
        &self,
        internals: &mut AgentInternals,
        messages: Vec<Message>,
        tool_definitions: &[crate::llm::ToolDefinition],
    ) -> Result<(Vec<ContentBlock>, Option<StopReason>)> {
        let response = self
            .llm
            .send_with_tools(
                messages,
                Some(&self.config.system_prompt),
                tool_definitions.to_vec(),
                None,
                self.config.thinking.clone(),
            )
            .await?;

        // Log API response if debugger is enabled
        if let Some(debugger) = internals.context.get_resource::<Debugger>() {
            if let Ok(response_json) = serde_json::to_value(&response) {
                if let Err(e) = debugger.log_api_response(&response_json) {
                    tracing::warn!("[StandardAgent] Failed to log API response: {}", e);
                }
            }
        }

        // Send text and thinking content to output
        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    internals.send_text(text);
                    internals.send_text_complete(text);
                }
                ContentBlock::Thinking { thinking, .. } => {
                    internals.send_thinking(thinking);
                    internals.send_thinking_complete(thinking);
                }
                _ => {}
            }
        }

        Ok((response.content, response.stop_reason))
    }

    /// Call LLM with streaming - sends deltas in real-time
    async fn call_llm_streaming(
        &self,
        internals: &mut AgentInternals,
        messages: Vec<Message>,
        tool_definitions: &[crate::llm::ToolDefinition],
    ) -> Result<(Vec<ContentBlock>, Option<StopReason>)> {
        let mut stream = self
            .llm
            .stream_with_tools(
                messages,
                Some(&self.config.system_prompt),
                tool_definitions.to_vec(),
                None,
                self.config.thinking.clone(),
            )
            .await?;

        // Track content blocks as they're built
        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut current_block_index: Option<usize> = None;
        let mut stop_reason: Option<StopReason> = None;

        // Accumulators for building content blocks
        let mut text_accum = String::new();
        let mut thinking_accum = String::new();
        let mut thinking_signature = String::new();
        let mut tool_input_accum = String::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    match event {
                        StreamEvent::MessageStart(_) => {
                            tracing::debug!("[StandardAgent] Stream started");
                        }

                        StreamEvent::ContentBlockStart(block_start) => {
                            current_block_index = Some(block_start.index);

                            match &block_start.content_block {
                                ContentBlockStart::Text { .. } => {
                                    text_accum.clear();
                                }
                                ContentBlockStart::Thinking { .. } => {
                                    thinking_accum.clear();
                                    thinking_signature.clear();
                                }
                                ContentBlockStart::ToolUse { id, name, .. } => {
                                    tool_input_accum.clear();
                                    current_tool_id = id.clone();
                                    current_tool_name = name.clone();
                                }
                            }
                        }

                        StreamEvent::ContentBlockDelta(delta) => {
                            match &delta.delta {
                                ContentDelta::TextDelta { text } => {
                                    text_accum.push_str(text);
                                    // Stream text to output immediately
                                    internals.send_text(text);
                                }
                                ContentDelta::ThinkingDelta { thinking } => {
                                    thinking_accum.push_str(thinking);
                                    // Stream thinking to output immediately
                                    internals.send_thinking(thinking);
                                }
                                ContentDelta::SignatureDelta { signature } => {
                                    thinking_signature.push_str(signature);
                                }
                                ContentDelta::InputJsonDelta { partial_json } => {
                                    tool_input_accum.push_str(partial_json);
                                }
                            }
                        }

                        StreamEvent::ContentBlockStop(block_stop) => {
                            if current_block_index == Some(block_stop.index) {
                                // Finalize the content block
                                if !text_accum.is_empty() {
                                    // Send text complete signal to CLI
                                    internals.send_text_complete(&text_accum);
                                    content_blocks.push(ContentBlock::Text {
                                        text: text_accum.clone(),
                                    });
                                    text_accum.clear();
                                } else if !thinking_accum.is_empty() {
                                    // Send thinking complete signal to CLI
                                    internals.send_thinking_complete(&thinking_accum);
                                    content_blocks.push(ContentBlock::Thinking {
                                        thinking: thinking_accum.clone(),
                                        signature: thinking_signature.clone(),
                                    });
                                    thinking_accum.clear();
                                    thinking_signature.clear();
                                } else if !tool_input_accum.is_empty()
                                    || !current_tool_name.is_empty()
                                {
                                    // Parse accumulated JSON
                                    let input: Value =
                                        serde_json::from_str(&tool_input_accum).unwrap_or_default();
                                    content_blocks.push(ContentBlock::ToolUse {
                                        id: current_tool_id.clone(),
                                        name: current_tool_name.clone(),
                                        input,
                                    });
                                    tool_input_accum.clear();
                                    current_tool_id.clear();
                                    current_tool_name.clear();
                                }
                                current_block_index = None;
                            }
                        }

                        StreamEvent::MessageDelta(msg_delta) => {
                            stop_reason = msg_delta.delta.stop_reason;
                        }

                        StreamEvent::MessageStop => {
                            tracing::debug!("[StandardAgent] Stream complete");
                        }

                        StreamEvent::Ping => {
                            tracing::trace!("[StandardAgent] Ping");
                        }

                        StreamEvent::Error(err) => {
                            tracing::error!(
                                "[StandardAgent] Stream error: {}: {}",
                                err.error.error_type,
                                err.error.message
                            );
                            internals.send_error(format!(
                                "Stream error: {}",
                                err.error.message
                            ));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("[StandardAgent] Stream error: {}", e);
                    return Err(e);
                }
            }
        }

        Ok((content_blocks, stop_reason))
    }
}
