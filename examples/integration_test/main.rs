//! Integration Test - Main Agent with FileSummaryAgent subagent
//!
//! This example demonstrates:
//! - A main agent with console attachment
//! - A custom SummarizeFile tool that spawns a subagent
//! - SubAgentManager for tracking spawned subagents
//! - Real-time streaming of subagent output to a file
//!
//! Run with: cargo run --example integration_test

mod file_summary_agent;
mod summarize_file_tool;

use std::io::Write;
use std::sync::Arc;

use anyhow::Result;

use shadow_agent_sdk::{
    agent::{AgentConfig, StandardAgent},
    core::OutputChunk,
    llm::AnthropicProvider,
    runtime::AgentRuntime,
    session::AgentSession,
    tools::ToolRegistry,
};

use summarize_file_tool::SummarizeFileTool;

/// System prompt for the main agent
const SYSTEM_PROMPT: &str = r#"You are a helpful assistant with access to file tools.

You have access to:
- SummarizeFile: Spawns a subagent to read and summarize any file

When the user asks about a file's contents, use SummarizeFile to get a summary.
Be helpful and concise."#;

/// Output file for streaming subagent tokens
const STREAMING_OUTPUT_FILE: &str = "subagent_stream.txt";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("integration_test=info,shadow_agent_sdk=info")
        .init();

    println!("=== Integration Test: Main Agent + Subagent Streaming ===");
    println!("This agent can spawn a FileSummaryAgent subagent.");
    println!("Subagent output will be streamed to: {}\n", STREAMING_OUTPUT_FILE);

    // --- Step 1: Create LLM provider ---
    println!("[Setup] Creating LLM provider...");
    let llm = Arc::new(AnthropicProvider::from_env()?);
    println!("[Setup] Model: {}", llm.model());

    // --- Step 2: Create runtime ---
    let runtime = AgentRuntime::new();
    println!("[Setup] Runtime created");

    // --- Step 3: Create tool registry with SummarizeFileTool ---
    let mut tools = ToolRegistry::new();
    tools.register(SummarizeFileTool::new(llm.clone(), runtime.clone()));
    let tools = Arc::new(tools);
    println!("[Setup] Tools registered: {:?}", tools.tool_names());

    // --- Step 4: Create session ---
    let session = AgentSession::new(
        "integration-test-main",
        "main-agent",
        "Main Agent",
        "Main agent that can spawn FileSummaryAgent subagents",
    )?;
    println!("[Setup] Session: {}", session.session_id());

    // --- Step 5: Configure and create agent ---
    let config = AgentConfig::new(SYSTEM_PROMPT)
        .with_tools(tools)
        .with_streaming(true) // Enable streaming for main agent too
        .with_debug(true);

    let agent = StandardAgent::new(config, llm);
    println!("[Setup] Agent configured with streaming enabled");

    // --- Step 6: Spawn the agent ---
    println!("[Setup] Spawning agent...");
    let handle = runtime
        .spawn(session, move |internals| agent.run(internals))
        .await;
    println!("[Setup] Agent spawned!");

    // --- Step 7: Run custom console loop with subagent monitoring ---
    println!();
    println!("Try: \"Summarize the file src/lib.rs\"");
    println!("Type 'exit' or 'quit' to stop.\n");

    // Clone handle for the subagent listener
    let runtime_for_listener = runtime.clone();

    loop {
        // Read user input
        print!("> ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        // Check for exit
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("[System] Shutting down...");
            break;
        }

        // Skip empty input
        if input.is_empty() {
            continue;
        }

        // Subscribe to main agent output BEFORE sending input
        let mut rx = handle.subscribe();

        // Send input to agent
        if let Err(e) = handle.send_input(input).await {
            eprintln!("[Error] Failed to send input: {}", e);
            continue;
        }

        // Process output with subagent detection
        let mut in_text = false;
        loop {
            match rx.recv().await {
                Ok(chunk) => {
                    match &chunk {
                        OutputChunk::TextDelta(text) => {
                            if !in_text {
                                print!("\n[Assistant] ");
                                in_text = true;
                            }
                            print!("{}", text);
                            std::io::stdout().flush()?;
                        }
                        OutputChunk::TextComplete(_) => {
                            if in_text {
                                println!();
                                in_text = false;
                            }
                        }
                        OutputChunk::ToolStart { name, .. } => {
                            println!("\n[Tool] Starting: {}", name);
                        }
                        OutputChunk::ToolEnd { result, .. } => {
                            if result.is_error {
                                println!("[Tool] Error: {}", result.output);
                            } else {
                                // Truncate long output
                                let output = if result.output.len() > 200 {
                                    format!("{}...", &result.output[..200])
                                } else {
                                    result.output.clone()
                                };
                                println!("[Tool] Result: {}", output);
                            }
                        }
                        OutputChunk::PermissionRequest { tool_name, action, input, .. } => {
                            println!("\n[Permission] Tool '{}' wants to: {}", tool_name, action);
                            println!("[Permission] Input: {}", input);
                            print!("[Permission] Allow? (y/n/a=always): ");
                            std::io::stdout().flush()?;

                            let mut response = String::new();
                            std::io::stdin().read_line(&mut response)?;
                            let response = response.trim().to_lowercase();

                            let (allowed, remember) = match response.as_str() {
                                "y" | "yes" => (true, false),
                                "a" | "always" => (true, true),
                                _ => (false, false),
                            };

                            if let Err(e) = handle.send_permission_response(tool_name.clone(), allowed, remember).await {
                                eprintln!("[Error] Failed to send permission response: {}", e);
                            }
                        }
                        OutputChunk::SubAgentSpawned { session_id, agent_type } => {
                            println!("\n[SubAgent] Spawned: {} ({})", agent_type, session_id);
                            println!("[SubAgent] Streaming output to: {}", STREAMING_OUTPUT_FILE);

                            // Start a task to listen to the subagent's output and write to file
                            let session_id_clone = session_id.clone();
                            let runtime_clone = runtime_for_listener.clone();

                            tokio::spawn(async move {
                                // Get the subagent handle from the runtime (should be registered immediately)
                                if let Some(subagent_handle) = runtime_clone.get(&session_id_clone).await {
                                    println!("[SubAgent] Got handle for {}, starting file writer...", session_id_clone);

                                    // Subscribe to subagent output
                                    let mut sub_rx = subagent_handle.subscribe();

                                    // Open file for writing
                                    let file = std::fs::File::create(STREAMING_OUTPUT_FILE);
                                    let mut file = match file {
                                        Ok(f) => f,
                                        Err(e) => {
                                            eprintln!("[SubAgent] Failed to create file: {}", e);
                                            return;
                                        }
                                    };

                                    // Write header
                                    let _ = writeln!(file, "=== Subagent {} Output ===\n", session_id_clone);

                                    // Stream tokens to file
                                    loop {
                                        match sub_rx.recv().await {
                                            Ok(chunk) => {
                                                match &chunk {
                                                    OutputChunk::TextDelta(text) => {
                                                        // Write token to file immediately
                                                        let _ = write!(file, "{}", text);
                                                        let _ = file.flush();
                                                        // Also print a dot to show progress
                                                        print!(".");
                                                        let _ = std::io::stdout().flush();
                                                    }
                                                    OutputChunk::ThinkingDelta(text) => {
                                                        let _ = write!(file, "[thinking] {}", text);
                                                        let _ = file.flush();
                                                    }
                                                    OutputChunk::Done => {
                                                        let _ = writeln!(file, "\n\n=== Done ===");
                                                        println!("\n[SubAgent] Finished streaming to file");
                                                        break;
                                                    }
                                                    OutputChunk::Error(e) => {
                                                        let _ = writeln!(file, "\n\n=== Error: {} ===", e);
                                                        eprintln!("\n[SubAgent] Error: {}", e);
                                                        break;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            Err(_) => {
                                                // Channel closed
                                                let _ = writeln!(file, "\n\n=== Channel closed ===");
                                                break;
                                            }
                                        }
                                    }
                                } else {
                                    eprintln!("[SubAgent] Could not find handle for {}", session_id_clone);
                                }
                            });
                        }
                        OutputChunk::SubAgentComplete { session_id, result } => {
                            println!("[SubAgent] {} completed", session_id);
                            if let Some(r) = result {
                                let summary = if r.len() > 100 {
                                    format!("{}...", &r[..100])
                                } else {
                                    r.clone()
                                };
                                println!("[SubAgent] Result preview: {}", summary);
                            }
                        }
                        OutputChunk::Done => {
                            if in_text {
                                println!();
                            }
                            break;
                        }
                        OutputChunk::Error(e) => {
                            println!("\n[Error] {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("[Error] Channel error: {}", e);
                    break;
                }
            }
        }

        println!();
    }

    // --- Cleanup ---
    println!("\n[Cleanup] Shutting down runtime...");
    runtime.shutdown_all().await;
    println!("[Cleanup] Done.");
    println!("[Cleanup] Check {} for streamed output!", STREAMING_OUTPUT_FILE);

    Ok(())
}
