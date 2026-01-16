//! Console Agent Example
//!
//! Demonstrates using the ConsoleRenderer with an agent.
//! The renderer is completely decoupled from the agent logic.
//!
//! Run with: cargo run --example console_agent

use anyhow::Result;
use singapore_project::{
    cli::ConsoleRenderer,
    core::InputMessage,
    llm::{AnthropicProvider, ContentBlock, Message},
    runtime::AgentRuntime,
    session::{AgentSession, SessionStorage},
};
use std::sync::Arc;

/// System prompt for the agent
const SYSTEM_PROMPT: &str = r#"You are a helpful coding assistant. Keep your responses concise and helpful.
When asked a question, provide a direct and clear answer."#;

/// The agent loop - completely independent of rendering
async fn agent_loop(
    mut internals: singapore_project::runtime::AgentInternals,
    llm: Arc<AnthropicProvider>,
) -> singapore_project::core::FrameworkResult<()> {
    loop {
        internals.set_idle().await;

        match internals.receive().await {
            Some(InputMessage::UserInput(text)) => {
                internals.set_processing().await;

                // Add user message to history
                internals.session.add_message(Message::user(&text))?;

                // Call LLM
                let messages = internals.session.history().to_vec();

                match llm
                    .send_with_tools(
                        messages,
                        Some(SYSTEM_PROMPT),
                        vec![],
                        None,
                        None,
                    )
                    .await
                {
                    Ok(response) => {
                        for block in &response.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    internals.send_text(text);
                                    internals.session.add_message(Message::assistant(text))?;
                                }
                                ContentBlock::Thinking { thinking, .. } => {
                                    internals.send_thinking(thinking);
                                }
                                _ => {}
                            }
                        }
                        internals.send_done();
                    }
                    Err(e) => {
                        internals.send_error(format!("LLM error: {}", e));
                    }
                }

                internals.session.save()?;
            }

            Some(InputMessage::Interrupt) => {
                internals.send_status("Interrupted");
                internals.set_done().await;
                break;
            }

            Some(InputMessage::Shutdown) | None => {
                internals.set_done().await;
                break;
            }

            _ => {}
        }

        internals.next_turn();
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize minimal logging
    tracing_subscriber::fmt()
        .with_env_filter("singapore_project=error")
        .init();

    // Create LLM provider
    let llm = Arc::new(AnthropicProvider::from_env()?);

    // Create runtime
    let runtime = AgentRuntime::new();

    // Create session storage
    let storage = SessionStorage::with_dir("./sessions");

    // Create session
    let session = AgentSession::new_with_storage(
        "console-session",
        "assistant",
        "Console Assistant",
        "An interactive console agent",
        storage,
    )?;

    // Spawn the agent
    let llm_clone = llm.clone();
    let handle = runtime
        .spawn(session, move |internals| agent_loop(internals, llm_clone))
        .await;

    // Create the console renderer - completely decoupled from agent
    let renderer = ConsoleRenderer::new(handle)
        .show_thinking(true)
        .show_tools(true);

    // Run the interactive console
    renderer.run().await?;

    Ok(())
}
