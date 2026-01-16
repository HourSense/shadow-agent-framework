//! Test Agent Example
//!
//! A minimal agent that demonstrates the framework by:
//! 1. Creating a session
//! 2. Spawning an agent with the runtime
//! 3. Wiring up the LLM for conversation
//! 4. Sending a test message and printing the response
//!
//! Run with: cargo run --example test_agent

use anyhow::Result;
use singapore_project::{
    core::{InputMessage, OutputChunk},
    llm::{AnthropicProvider, ContentBlock, Message},
    runtime::AgentRuntime,
    session::{AgentSession, SessionStorage},
};
use std::sync::Arc;

/// System prompt for the test agent
const SYSTEM_PROMPT: &str = r#"You are a helpful assistant. Keep your responses concise and friendly.
When asked a question, provide a direct and clear answer."#;

/// The agent loop - this is what the programmer writes
async fn agent_loop(
    mut internals: singapore_project::runtime::AgentInternals,
    llm: Arc<AnthropicProvider>,
) -> singapore_project::core::FrameworkResult<()> {
    println!("[Agent] Started, waiting for input...");

    loop {
        // Wait for input
        internals.set_idle().await;

        match internals.receive().await {
            Some(InputMessage::UserInput(text)) => {
                println!("[Agent] Received input: {}", text);

                // Update state
                internals.set_processing().await;

                // Add user message to session history
                internals.session.add_message(Message::user(&text))?;

                // Call the LLM
                println!("[Agent] Calling LLM...");
                let messages = internals.session.history().to_vec();

                match llm
                    .send_with_tools(
                        messages,
                        Some(SYSTEM_PROMPT),
                        vec![], // No tools for this simple test
                        None,
                        None, // No extended thinking for simplicity
                    )
                    .await
                {
                    Ok(response) => {
                        println!("[Agent] Got response from LLM");

                        // Process response blocks
                        for block in &response.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    // Stream the text
                                    internals.send_text(text);

                                    // Add to history
                                    internals
                                        .session
                                        .add_message(Message::assistant(text))?;
                                }
                                ContentBlock::Thinking { thinking, .. } => {
                                    // Stream thinking (if present)
                                    internals.send_thinking(thinking);
                                }
                                ContentBlock::ToolUse { id, name, input } => {
                                    // For this simple test, we just note tool requests
                                    println!(
                                        "[Agent] Tool requested: {} (id: {}), input: {}",
                                        name, id, input
                                    );
                                    internals.send(OutputChunk::ToolStart {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: input.clone(),
                                    });
                                }
                                _ => {}
                            }
                        }

                        // Signal done
                        internals.send_done();
                    }
                    Err(e) => {
                        println!("[Agent] LLM error: {}", e);
                        internals.send_error(format!("LLM error: {}", e));
                    }
                }

                // Save session
                internals.session.save()?;
            }

            Some(InputMessage::Interrupt) => {
                println!("[Agent] Interrupted");
                internals.send_status("Interrupted");
                internals.set_done().await;
                break;
            }

            Some(InputMessage::Shutdown) | None => {
                println!("[Agent] Shutting down");
                internals.set_done().await;
                break;
            }

            other => {
                println!("[Agent] Received other message: {:?}", other);
            }
        }

        // Increment turn counter
        internals.next_turn();
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging (only show errors unless RUST_LOG is set)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("singapore_project=error".parse().unwrap()),
        )
        .init();

    println!("=== Test Agent Example ===\n");

    // Create LLM provider
    println!("[Main] Creating LLM provider...");
    let llm = Arc::new(AnthropicProvider::from_env()?);
    println!("[Main] Using model: {}", llm.model());

    // Create runtime
    let runtime = AgentRuntime::new();

    // Create session storage in a persistent directory
    let storage = SessionStorage::with_dir("./sessions");

    // Create session
    println!("[Main] Creating session...");
    let session = AgentSession::new_with_storage(
        "test-session-001",
        "test-agent",
        "Test Agent",
        "A simple test agent for framework validation",
        storage,
    )?;

    println!("[Main] Session ID: {}", session.session_id());

    // Spawn the agent
    println!("[Main] Spawning agent...");
    let llm_clone = llm.clone();
    let handle = runtime
        .spawn(session, move |internals| agent_loop(internals, llm_clone))
        .await;

    // Subscribe to output
    let mut rx = handle.subscribe();

    // Send a test message
    let test_message = "Hello! Can you briefly explain what makes a good software framework?";
    println!("\n[Main] Sending message: {}\n", test_message);
    handle.send_input(test_message).await?;

    // Collect and print the response
    println!("--- Response ---");
    loop {
        match rx.recv().await {
            Ok(chunk) => match chunk {
                OutputChunk::TextDelta(text) => {
                    print!("{}", text);
                }
                OutputChunk::ThinkingDelta(text) => {
                    // Optionally show thinking
                    print!("[thinking: {}]", text);
                }
                OutputChunk::StateChange(state) => {
                    println!("\n[State: {:?}]", state);
                }
                OutputChunk::Done => {
                    println!("\n--- End of Response ---");
                    break;
                }
                OutputChunk::Error(e) => {
                    println!("\n[Error: {}]", e);
                    break;
                }
                _ => {}
            },
            Err(e) => {
                println!("\n[Channel error: {}]", e);
                break;
            }
        }
    }

    // Shutdown the agent
    println!("\n[Main] Shutting down agent...");
    handle.shutdown().await?;

    // Wait for cleanup
    runtime.wait_for("test-session-001").await?;
    println!("[Main] Agent stopped.");

    println!("\n=== Test Complete ===");
    Ok(())
}
