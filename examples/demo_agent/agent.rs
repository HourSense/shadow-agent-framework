//! Agent Definition
//!
//! This module defines the agent's execution loop.
//! The agent runs in its own tokio task, completely independent of the renderer.

use singapore_project::{
    core::InputMessage,
    llm::{AnthropicProvider, ContentBlock, Message},
    runtime::AgentInternals,
};
use std::sync::Arc;

/// System prompt for the demo agent
pub const SYSTEM_PROMPT: &str = r#"You are a helpful coding assistant.
Keep your responses concise and helpful.
When asked a question, provide a direct and clear answer."#;

/// The agent's main execution loop
///
/// This function defines what the agent does:
/// 1. Wait for input
/// 2. Process input (call LLM)
/// 3. Send output
/// 4. Repeat until shutdown
///
/// The programmer has full control over this loop.
pub async fn run(
    mut internals: AgentInternals,
    llm: Arc<AnthropicProvider>,
) -> singapore_project::core::FrameworkResult<()> {
    tracing::info!("[Agent] Started, waiting for input...");

    loop {
        // Signal we're ready for input
        internals.set_idle().await;

        // Wait for next message
        match internals.receive().await {
            Some(InputMessage::UserInput(text)) => {
                tracing::info!("[Agent] Received: {}", text);

                // Update state to processing
                internals.set_processing().await;

                // Add user message to session history
                internals.session.add_message(Message::user(&text))?;

                // Call the LLM
                tracing::info!("[Agent] Calling LLM...");
                let messages = internals.session.history().to_vec();

                match llm
                    .send_with_tools(
                        messages,
                        Some(SYSTEM_PROMPT),
                        vec![], // No tools for this demo
                        None,
                        None,
                    )
                    .await
                {
                    Ok(response) => {
                        tracing::info!("[Agent] Got LLM response");

                        // Process response blocks
                        for block in &response.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    // Stream text to renderer
                                    internals.send_text(text);
                                    // Save to history
                                    internals.session.add_message(Message::assistant(text))?;
                                }
                                ContentBlock::Thinking { thinking, .. } => {
                                    // Stream thinking to renderer
                                    internals.send_thinking(thinking);
                                }
                                _ => {}
                            }
                        }

                        // Signal turn complete
                        internals.send_done();
                    }
                    Err(e) => {
                        tracing::error!("[Agent] LLM error: {}", e);
                        internals.send_error(format!("LLM error: {}", e));
                    }
                }

                // Persist session
                internals.session.save()?;
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
                // Ignore other message types for this demo
            }
        }

        // Increment turn counter
        internals.next_turn();
    }

    Ok(())
}
