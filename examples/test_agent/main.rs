//! Test Agent Example
//!
//! Demonstrates the full agent framework:
//! - Runtime with shared global permissions
//! - Agent with permission-aware tool execution
//! - Console renderer for user interaction
//! - Read, Write, Bash, and TodoWrite tools
//! - TodoListManager for task tracking
//!
//! Read operations are pre-allowed, others will prompt the user.
//!
//! Run with: cargo run --example test_agent

mod agent;
mod tools;

use anyhow::Result;
use std::sync::Arc;

use singapore_project::{
    cli::ConsoleRenderer,
    helpers::TodoListManager,
    llm::AnthropicProvider,
    permissions::PermissionRule,
    runtime::AgentRuntime,
    session::{AgentSession, SessionStorage},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("test_agent=info,singapore_project=warn")
        .init();

    println!("=== Test Agent ===");
    println!("This agent has access to Read, Write, Bash, and TodoWrite tools.");
    println!("Read operations are pre-allowed. Others will require your permission.\n");

    // --- Step 1: Create LLM provider ---
    println!("[Setup] Creating LLM provider...");
    let llm = Arc::new(AnthropicProvider::from_env()?);
    println!("[Setup] Model: {}", llm.model());

    // --- Step 2: Create runtime with global Read permission ---
    let runtime = AgentRuntime::new();
    runtime
        .global_permissions()
        .add_rule(PermissionRule::allow_tool("Read"));
    println!("[Setup] Runtime created (Read tool globally allowed)");

    // --- Step 3: Create tool registry ---
    let tools = Arc::new(tools::create_registry()?);
    println!("[Setup] Tools registered: {:?}", tools.tool_names());

    // --- Step 4: Create session with persistent storage ---
    let storage = SessionStorage::with_dir("./sessions");
    let session = AgentSession::new_with_storage(
        "test-agent-session",
        "test-agent",
        "Test Agent",
        "A test agent demonstrating the permission system",
        storage,
    )?;
    println!("[Setup] Session: {}", session.session_id());

    // --- Step 5: Create TodoListManager (shared between agent and console) ---
    let todo_manager = Arc::new(TodoListManager::new());
    println!("[Setup] TodoListManager created");

    // --- Step 6: Spawn the agent ---
    println!("[Setup] Spawning agent...");
    let llm_clone = llm.clone();
    let tools_clone = tools.clone();
    let todo_clone = todo_manager.clone();
    let handle = runtime
        .spawn(session, move |internals| {
            agent::run(internals, llm_clone, tools_clone, todo_clone)
        })
        .await;
    println!("[Setup] Agent spawned!");

    // --- Step 7: Create and run the console renderer ---
    println!("[Setup] Starting console renderer...");
    println!();
    println!("Type your requests below. Read is pre-allowed, others will ask for permission.");
    println!("Type 'exit' or 'quit' to stop.\n");

    let renderer = ConsoleRenderer::new(handle)
        .show_thinking(true)
        .show_tools(true)
        .with_todo_manager(todo_manager);

    // Run the console - this blocks until user types "exit"
    renderer.run().await?;

    // --- Cleanup ---
    println!("\n[Cleanup] Shutting down runtime...");
    runtime.shutdown_all().await;

    println!("[Cleanup] Done.");
    Ok(())
}
