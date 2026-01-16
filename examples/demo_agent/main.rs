//! Demo Agent Example
//!
//! Demonstrates the framework architecture:
//! - Agent runs in its own tokio task (agent.rs)
//! - Main initializes and manages the agent
//! - Console renderer is opt-in and decoupled
//!
//! Run with: cargo run --example demo_agent

mod agent;

use anyhow::Result;
use singapore_project::{
    cli::ConsoleRenderer,
    llm::AnthropicProvider,
    runtime::AgentRuntime,
    session::{AgentSession, SessionStorage},
};
use std::sync::Arc;
use tokio::time::{interval, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging - show info level for our demo
    tracing_subscriber::fmt()
        .with_env_filter("demo_agent=info,singapore_project=warn")
        .init();

    println!("=== Demo Agent ===\n");

    // --- Step 1: Create LLM provider ---
    println!("[Main] Creating LLM provider...");
    let llm = Arc::new(AnthropicProvider::from_env()?);
    println!("[Main] Using model: {}", llm.model());

    // --- Step 2: Create runtime ---
    let runtime = AgentRuntime::new();

    // --- Step 3: Create session with persistent storage ---
    let storage = SessionStorage::with_dir("./sessions");
    let session = AgentSession::new_with_storage(
        "demo-agent-session",
        "demo-agent",
        "Demo Agent",
        "A demonstration agent for testing the framework",
        storage,
    )?;
    println!("[Main] Session: {}", session.session_id());

    // --- Step 4: Spawn the agent ---
    // The agent runs in its own tokio task, completely independent
    println!("[Main] Spawning agent...");
    let llm_clone = llm.clone();
    let handle = runtime
        .spawn(session, move |internals| agent::run(internals, llm_clone))
        .await;
    println!("[Main] Agent spawned!");

    // --- Step 5: Prove main is not blocked ---
    // Spawn a background task that prints "main is active" periodically
    let activity_handle = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(2));
        loop {
            ticker.tick().await;
            println!("[Main] main is active - proves main loop is not blocked");
        }
    });

    // --- Step 6: Create and run the console renderer ---
    // This is opt-in - the programmer chooses to use it
    println!("[Main] Starting console renderer...\n");
    let renderer = ConsoleRenderer::new(handle)
        .show_thinking(true)
        .show_tools(true);

    // Run the console - this blocks until user types "exit"
    // But the agent and activity ticker run independently
    renderer.run().await?;

    // --- Cleanup ---
    println!("\n[Main] Stopping activity ticker...");
    activity_handle.abort();

    println!("[Main] Shutting down runtime...");
    runtime.shutdown_all().await;

    println!("[Main] Done.");
    Ok(())
}
