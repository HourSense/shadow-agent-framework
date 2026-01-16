//! Agent loop with permission-aware tool execution
//!
//! This agent:
//! 1. Receives user input
//! 2. Calls LLM with tools
//! 3. For each tool_use, checks permission before executing
//! 4. If no permission rule exists, prompts user via renderer
//! 5. Executes tool if allowed, continues LLM loop if needed

use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use singapore_project::{
    core::{FrameworkResult, InputMessage},
    helpers::TodoListManager,
    llm::{AnthropicProvider, ContentBlock, Message, StopReason},
    permissions::{CheckResult, PermissionRule, PermissionScope},
    runtime::AgentInternals,
    tools::{ToolRegistry, ToolResult},
};

/// System prompt for the test agent
const SYSTEM_PROMPT: &str = r#"You are a helpful coding assistant with access to tools.

You have the following tools available:
- Read: Read file contents
- Write: Write or create files
- Bash: Execute shell commands
- TodoWrite: Track tasks you need to perform

When the user asks you to do something, use the appropriate tools.
Use TodoWrite to track multi-step tasks and show progress.
Be concise in your responses."#;

/// Run the agent loop
pub async fn run(
    mut internals: AgentInternals,
    llm: Arc<AnthropicProvider>,
    tools: Arc<ToolRegistry>,
    todo_manager: Arc<TodoListManager>,
) -> FrameworkResult<()> {
    // Insert TodoListManager into context so TodoWriteTool can find it
    internals.context.insert_resource(todo_manager);

    tracing::info!("[Agent] Started, waiting for input...");

    loop {
        // Signal we're ready for input
        internals.set_idle().await;

        // Wait for next message
        match internals.receive().await {
            Some(InputMessage::UserInput(text)) => {
                tracing::info!("[Agent] Received: {}", text);
                internals.set_processing().await;

                // Process the user message
                if let Err(e) = process_turn(&mut internals, &llm, &tools, &text).await {
                    tracing::error!("[Agent] Error processing turn: {}", e);
                    internals.send_error(format!("Error: {}", e));
                }

                // Signal turn complete
                internals.send_done();

                // Persist session
                if let Err(e) = internals.session.save() {
                    tracing::error!("[Agent] Failed to save session: {}", e);
                }
            }

            Some(InputMessage::Interrupt) => {
                tracing::info!("[Agent] Interrupted");
                internals.send_status("Interrupted");
                internals.set_done().await;
                break;
            }

            Some(InputMessage::Shutdown) | None => {
                tracing::info!("[Agent] Shutting down");
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

/// Process a single user turn (may involve multiple LLM calls for tool use)
async fn process_turn(
    internals: &mut AgentInternals,
    llm: &AnthropicProvider,
    tools: &ToolRegistry,
    user_input: &str,
) -> Result<()> {
    // Add user message to history
    internals
        .session
        .add_message(Message::user(user_input))?;

    // Get tool definitions
    let tool_definitions = tools.get_definitions();

    // LLM loop - continues until no more tool calls
    loop {
        // Get messages for LLM
        let messages = internals.session.history().to_vec();

        tracing::info!("[Agent] Calling LLM with {} messages", messages.len());

        // Call LLM
        let response = llm
            .send_with_tools(
                messages,
                Some(SYSTEM_PROMPT),
                tool_definitions.clone(),
                None,
                None,
            )
            .await?;

        tracing::info!("[Agent] LLM response: stop_reason={:?}", response.stop_reason);

        // Process response content blocks
        let mut tool_results: Vec<(String, ToolResult)> = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    internals.send_text(text);
                }

                ContentBlock::Thinking { thinking, .. } => {
                    internals.send_thinking(thinking);
                }

                ContentBlock::ToolUse { id, name, input } => {
                    tracing::info!("[Agent] Tool use: {} ({})", name, id);

                    // Execute tool with permission check
                    let result = execute_tool_with_permission(internals, tools, name, input).await;

                    tool_results.push((id.clone(), result));
                }

                _ => {}
            }
        }

        // Add assistant message to history
        internals
            .session
            .add_message(Message::assistant_with_blocks(response.content.clone()))?;

        // If there were tool calls, add results and continue loop
        if !tool_results.is_empty() {
            // Add tool results as a message
            let tool_result_blocks: Vec<ContentBlock> = tool_results
                .into_iter()
                .map(|(id, result)| ContentBlock::tool_result(&id, &result.output, result.is_error))
                .collect();

            internals
                .session
                .add_message(Message::user_with_blocks(tool_result_blocks))?;

            // Continue to next LLM call
            continue;
        }

        // No tool calls - check if we should stop
        match response.stop_reason {
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

/// Execute a tool with permission checking
async fn execute_tool_with_permission(
    internals: &mut AgentInternals,
    tools: &ToolRegistry,
    tool_name: &str,
    input: &Value,
) -> ToolResult {
    let input_str = input.to_string();

    // Get tool info for better permission prompts
    let tool_info = tools.get_tool_info(tool_name, input);
    let action_desc = tool_info
        .as_ref()
        .map(|i| i.action_description.clone())
        .unwrap_or_else(|| format!("Execute {}", tool_name));

    // Check permission
    match internals.check_permission(tool_name, &input_str) {
        CheckResult::Allowed => {
            tracing::info!("[Agent] Permission allowed for {}", tool_name);
            execute_tool(internals, tools, tool_name, input).await
        }

        CheckResult::Denied => {
            tracing::info!("[Agent] Permission denied for {}", tool_name);
            ToolResult::error(format!("Permission denied for tool: {}", tool_name))
        }

        CheckResult::AskUser => {
            tracing::info!("[Agent] Asking user for permission: {}", tool_name);

            // Send permission request
            internals.send_permission_request(
                tool_name,
                &action_desc,
                &input_str,
                tool_info.and_then(|i| i.details),
            );
            internals.set_waiting_for_permission().await;

            // Wait for response
            match internals.receive().await {
                Some(InputMessage::PermissionResponse {
                    tool_name: resp_tool,
                    allowed,
                    remember,
                }) => {
                    if resp_tool != tool_name {
                        tracing::warn!(
                            "[Agent] Permission response mismatch: expected {}, got {}",
                            tool_name,
                            resp_tool
                        );
                        return ToolResult::error("Permission response mismatch");
                    }

                    if remember && allowed {
                        tracing::info!("[Agent] Adding 'Always Allow' rule for {}", tool_name);
                        internals.add_permission_rule(
                            PermissionRule::allow_tool(tool_name),
                            PermissionScope::Session,
                        );
                    }

                    if allowed {
                        tracing::info!("[Agent] User allowed {}", tool_name);
                        execute_tool(internals, tools, tool_name, input).await
                    } else {
                        tracing::info!("[Agent] User denied {}", tool_name);
                        ToolResult::error(format!("User denied permission for: {}", tool_name))
                    }
                }

                Some(InputMessage::Interrupt) => {
                    tracing::info!("[Agent] Interrupted while waiting for permission");
                    ToolResult::error("Interrupted")
                }

                Some(InputMessage::Shutdown) => {
                    tracing::info!("[Agent] Shutdown while waiting for permission");
                    ToolResult::error("Shutdown")
                }

                None => {
                    tracing::info!("[Agent] Channel closed while waiting for permission");
                    ToolResult::error("Channel closed")
                }

                _ => {
                    tracing::warn!("[Agent] Unexpected message while waiting for permission");
                    ToolResult::error("Unexpected message during permission request")
                }
            }
        }
    }
}

/// Execute a tool (after permission is granted)
async fn execute_tool(
    internals: &mut AgentInternals,
    tools: &ToolRegistry,
    tool_name: &str,
    input: &Value,
) -> ToolResult {
    // Update state
    internals
        .set_executing_tool(tool_name, "")
        .await;

    // Send tool start notification
    internals.send_tool_start(tool_name, tool_name, input.clone());

    // Execute
    let result = match tools.execute(tool_name, input, internals).await {
        Ok(result) => result,
        Err(e) => ToolResult::error(format!("Tool execution failed: {}", e)),
    };

    // Send tool end notification
    internals.send_tool_end(tool_name, result.clone());

    result
}
