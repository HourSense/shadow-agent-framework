# Shadow Agent SDK

A Rust framework for building AI agents with Claude. Designed for applications that need to spawn, manage, and communicate with autonomous agents - particularly suited for Tauri apps and other frontend-backend architectures.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Core Concepts](#core-concepts)
3. [Module Reference](#module-reference)
   - [Runtime](#runtime-module)
   - [Session](#session-module)
   - [Tools](#tools-module)
   - [Permissions](#permissions-module)
   - [Hooks](#hooks-module)
   - [LLM Provider](#llm-module)
   - [Helpers](#helpers-module)
4. [Building a Frontend Integration](#building-a-frontend-integration)
5. [Examples](#examples)
6. [API Reference](#api-reference)

---

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
shadow-agent-sdk = { path = "path/to/shadow-agent-sdk" }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
```

### Minimal Example

```rust
use std::sync::Arc;
use shadow_agent_sdk::{
    agent::{AgentConfig, StandardAgent},
    llm::AnthropicProvider,
    runtime::AgentRuntime,
    session::AgentSession,
    tools::ToolRegistry,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Create LLM provider (reads ANTHROPIC_API_KEY from env)
    let llm = Arc::new(AnthropicProvider::from_env()?);

    // 2. Create runtime (manages all agents)
    let runtime = AgentRuntime::new();

    // 3. Create session (persists conversation)
    let session = AgentSession::new(
        "my-session",      // unique ID
        "assistant",       // agent type
        "My Assistant",    // display name
        "A helpful agent", // description
    )?;

    // 4. Configure and create agent
    let config = AgentConfig::new("You are a helpful assistant.")
        .with_streaming(true);
    let agent = StandardAgent::new(config, llm);

    // 5. Spawn agent
    let handle = runtime
        .spawn(session, |internals| agent.run(internals))
        .await;

    // 6. Send input and receive output
    let mut rx = handle.subscribe();
    handle.send_input("Hello!").await?;

    // 7. Process output stream
    loop {
        match rx.recv().await {
            Ok(chunk) => {
                use shadow_agent_sdk::core::OutputChunk;
                match chunk {
                    OutputChunk::TextDelta(text) => print!("{}", text),
                    OutputChunk::Done => break,
                    OutputChunk::Error(e) => eprintln!("Error: {}", e),
                    _ => {}
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}
```

---

## Core Concepts

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Your Application                          │
│  (Tauri, CLI, Web Server, etc.)                                 │
└─────────────────────┬───────────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                       AgentRuntime                               │
│  - Spawns and manages agent tasks                               │
│  - Maintains registry of running agents                         │
│  - Shares global permissions across agents                      │
└─────────────────────┬───────────────────────────────────────────┘
                      │ spawn()
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                       AgentHandle                                │
│  - External interface for communication                         │
│  - send_input(), subscribe(), state()                           │
│  - Cloneable, shareable across threads                          │
└─────────────────────┬───────────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                      StandardAgent                               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Agent Loop:                                               │   │
│  │  1. Receive input → 2. Inject context                    │   │
│  │  3. Call LLM → 4. Parse response                         │   │
│  │  5. Execute tools (with permissions) → 6. Send output    │   │
│  │  7. Repeat until done                                    │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Components:                                                     │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐   │
│  │   Tools    │ │ Permissions│ │   Hooks    │ │  Session   │   │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Key Types

| Type | Purpose |
|------|---------|
| `AgentRuntime` | Spawns and manages agent lifecycles |
| `AgentHandle` | Communicate with running agents |
| `AgentSession` | Conversation state and persistence |
| `AgentConfig` | Configure agent behavior |
| `StandardAgent` | Complete agent implementation |
| `ToolRegistry` | Manages available tools |
| `PermissionManager` | Three-tier permission system |
| `HookRegistry` | Intercept and modify agent behavior |
| `AnthropicProvider` | LLM API client |

### Agent States

```rust
pub enum AgentState {
    Idle,                           // Waiting for input
    Processing,                     // Calling LLM
    WaitingForPermission,          // Awaiting user approval
    ExecutingTool { tool_name, tool_use_id },
    WaitingForSubAgent { session_id },
    Done,
    Error { message },
}
```

### Communication Flow

```
Frontend                    AgentHandle                 Agent
   │                            │                         │
   │── send_input("Hello") ────►│                         │
   │                            │── UserInput ───────────►│
   │                            │                         │── Process
   │                            │◄── TextDelta ──────────│
   │◄── OutputChunk ───────────│                         │
   │                            │◄── ToolStart ──────────│
   │◄── OutputChunk ───────────│                         │
   │                            │◄── PermissionRequest ──│
   │◄── OutputChunk ───────────│                         │
   │                            │                         │
   │── send_permission ────────►│── PermissionResponse ──►│
   │                            │                         │── Execute Tool
   │                            │◄── ToolEnd ────────────│
   │◄── OutputChunk ───────────│                         │
   │                            │◄── Done ───────────────│
   │◄── OutputChunk ───────────│                         │
```

---

## Module Reference

### Runtime Module

The runtime manages agent lifecycles and provides handles for communication.

#### AgentRuntime

```rust
use shadow_agent_sdk::runtime::AgentRuntime;

// Create runtime
let runtime = AgentRuntime::new();

// Or with global permission rules
let runtime = AgentRuntime::with_global_rules(vec![
    PermissionRule::allow_tool("Read"),
]);

// Spawn an agent
let handle = runtime.spawn(session, |internals| agent.run(internals)).await;

// Spawn with local permission rules
let handle = runtime.spawn_with_local_rules(
    session,
    vec![PermissionRule::allow_tool("Read")],
    |internals| agent.run(internals),
).await;

// Get existing agent
if let Some(handle) = runtime.get("session-id").await {
    handle.send_input("Hello").await?;
}

// List running agents
let running = runtime.list_running().await; // Vec<String>

// Shutdown
runtime.shutdown("session-id").await;
runtime.shutdown_all().await;
```

#### AgentHandle

```rust
use shadow_agent_sdk::runtime::AgentHandle;
use shadow_agent_sdk::core::OutputChunk;

// Send input
handle.send_input("Write hello world").await?;

// Send permission response
handle.send_permission_response("Bash".to_string(), true, false).await?;
// (tool_name, allowed, remember)

// Subscribe to output (do this BEFORE sending input!)
let mut rx = handle.subscribe();

// Process output
while let Ok(chunk) = rx.recv().await {
    match chunk {
        OutputChunk::TextDelta(text) => print!("{}", text),
        OutputChunk::TextComplete(full_text) => {},
        OutputChunk::ThinkingDelta(text) => {},
        OutputChunk::ToolStart { id, name, input } => {},
        OutputChunk::ToolEnd { id, result } => {},
        OutputChunk::PermissionRequest { tool_name, action, input, details } => {
            // Show UI prompt, then:
            handle.send_permission_response(tool_name, true, false).await?;
        },
        OutputChunk::SubAgentSpawned { session_id, agent_type } => {},
        OutputChunk::SubAgentComplete { session_id, result } => {},
        OutputChunk::StateChange(state) => {},
        OutputChunk::Error(message) => {},
        OutputChunk::Done => break,
        _ => {}
    }
}

// Check state
let state = handle.state().await;
let is_busy = handle.is_processing().await;
let is_done = handle.is_done().await;

// Control
handle.interrupt().await;  // Cancel current operation
handle.shutdown().await;   // Stop agent entirely
```

### Session Module

Sessions persist conversation history and metadata.

#### AgentSession

```rust
use shadow_agent_sdk::session::{AgentSession, SessionStorage};

// Create new session (auto-persists to ./sessions/{id}/)
let session = AgentSession::new(
    "unique-session-id",
    "coder",              // agent_type
    "Code Assistant",     // name
    "Helps with coding",  // description
)?;

// Create subagent session (linked to parent)
let subagent_session = AgentSession::new_subagent(
    "sub-session-id",
    "researcher",
    "Research Helper",
    "Finds information",
    "parent-session-id",  // parent_session_id
    "tool_use_123",       // parent_tool_use_id
)?;

// Load existing session
let session = AgentSession::load("session-id")?;

// Access properties
let id = session.session_id();
let history = session.history();  // &[Message]
let is_sub = session.is_subagent();
let parent = session.parent_session_id();  // Option<&str>
let children = session.child_session_ids();  // &[String]

// Add message manually
session.add_message(Message::user("Hello"))?;

// Save changes
session.save()?;

// Delete session
session.delete()?;
```

#### Listing Sessions

```rust
// List all session IDs
let all = AgentSession::list_all()?;

// List only top-level sessions (not subagents)
let top_level = AgentSession::list_top_level()?;

// List with filter
let filtered = AgentSession::list_filtered(true)?;  // true = top-level only

// List with metadata (more efficient if you need metadata)
let with_meta = AgentSession::list_with_metadata(true)?;
// Returns: Vec<(String, SessionMetadata)>

for (session_id, metadata) in with_meta {
    println!("{}: {} ({})", session_id, metadata.name, metadata.agent_type);
    println!("  Created: {}", metadata.created_at);
    println!("  Is subagent: {}", metadata.is_subagent());
}
```

#### Getting History

```rust
// Get conversation history without loading full session
let history = AgentSession::get_history("session-id")?;
// Returns: Vec<Message>

// Get just metadata
let metadata = AgentSession::get_metadata("session-id")?;
// Returns: SessionMetadata

// Check existence
if AgentSession::exists("session-id") {
    // ...
}
```

#### Custom Storage Location

```rust
let storage = SessionStorage::with_dir("/custom/path");
let session = AgentSession::new_with_storage(
    "session-id", "type", "name", "desc", storage
)?;
```

### Tools Module

Tools extend agent capabilities with custom actions.

#### Built-in Tools

```rust
use shadow_agent_sdk::tools::{ToolRegistry, common::*};

let mut tools = ToolRegistry::new();

// File operations
tools.register(ReadTool::new()?);       // Read file contents
tools.register(WriteTool::new()?);      // Write/create files
tools.register(EditTool::new()?);       // Edit existing files
tools.register(GlobTool::new()?);       // Find files by pattern
tools.register(GrepTool::new()?);       // Search file contents

// Shell
tools.register(BashTool::new()?);       // Execute commands

// Task management
tools.register(TodoWriteTool::new()?);  // Manage task lists

let tools = Arc::new(tools);
```

#### Creating Custom Tools

```rust
use async_trait::async_trait;
use shadow_agent_sdk::tools::{Tool, ToolResult, ToolInfo};
use shadow_agent_sdk::llm::{ToolDefinition, types::CustomTool};
use shadow_agent_sdk::runtime::AgentInternals;
use serde_json::{json, Value};

pub struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "GetWeather"
    }

    fn description(&self) -> &str {
        "Get current weather for a location"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::Custom(CustomTool {
            name: self.name().to_string(),
            description: Some(self.description().to_string()),
            input_schema: shadow_agent_sdk::llm::types::ToolInputSchema {
                schema_type: "object".to_string(),
                properties: Some(json!({
                    "location": {
                        "type": "string",
                        "description": "City name or coordinates"
                    }
                })),
                required: Some(vec!["location".to_string()]),
            },
            tool_type: None,
        })
    }

    fn get_info(&self, input: &Value) -> ToolInfo {
        let location = input.get("location")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        ToolInfo {
            name: self.name().to_string(),
            action_description: format!("Get weather for {}", location),
            details: None,
        }
    }

    fn requires_permission(&self) -> bool {
        false  // Safe tool, no permission needed
    }

    async fn execute(
        &self,
        input: &Value,
        internals: &mut AgentInternals,
    ) -> anyhow::Result<ToolResult> {
        let location = input.get("location")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing location"))?;

        // Access agent context if needed
        let session_id = internals.context.session_id.clone();

        // Your implementation here...
        let weather = fetch_weather(location).await?;

        Ok(ToolResult::success(format!("Weather in {}: {}", location, weather)))
    }
}
```

#### Tools That Spawn Subagents

```rust
async fn execute(&self, input: &Value, internals: &mut AgentInternals) -> Result<ToolResult> {
    let parent_session = internals.session.session_id().to_string();
    let tool_use_id = internals.context.current_tool_use_id
        .clone()
        .unwrap_or_default();

    // Create subagent session
    let sub_session = AgentSession::new_subagent(
        format!("sub-{}", tool_use_id),
        "researcher",
        "Research Agent",
        "Researches topics",
        &parent_session,
        &tool_use_id,
    )?;

    // Notify parent of spawn
    internals.send(OutputChunk::SubAgentSpawned {
        session_id: sub_session.session_id().to_string(),
        agent_type: "researcher".to_string(),
    });

    // Get runtime from context and spawn
    if let Some(runtime) = internals.context.get_resource::<AgentRuntime>() {
        let config = AgentConfig::new("You are a researcher...");
        let agent = StandardAgent::new(config, self.llm.clone());

        let handle = runtime
            .spawn(sub_session, |internals| agent.run(internals))
            .await;

        // Send task to subagent
        handle.send_input("Research topic X").await?;

        // Wait for completion
        let mut rx = handle.subscribe();
        let mut result = String::new();

        while let Ok(chunk) = rx.recv().await {
            match chunk {
                OutputChunk::TextDelta(text) => result.push_str(&text),
                OutputChunk::Done => break,
                _ => {}
            }
        }

        // Notify completion
        internals.send(OutputChunk::SubAgentComplete {
            session_id: handle.session_id().to_string(),
            result: Some(result.clone()),
        });

        Ok(ToolResult::success(result))
    } else {
        Ok(ToolResult::error("Runtime not available"))
    }
}
```

### Permissions Module

Three-tier permission system for controlling tool access.

#### Permission Tiers

```
1. Session Rules   - Highest priority, in-memory only
2. Local Rules     - Agent-type specific, persisted
3. Global Rules    - All agents, persisted

Check order: Session → Local → Global → Ask User (if interactive)
```

#### Permission Rules

```rust
use shadow_agent_sdk::permissions::{PermissionRule, PermissionScope};

// Allow entire tool
let rule = PermissionRule::allow_tool("Read");

// Allow commands with specific prefix
let rule = PermissionRule::allow_prefix("Bash", "git ");
let rule = PermissionRule::allow_prefix("Bash", "npm ");

// Add rules at different scopes
runtime.global_permissions().add_rule(
    PermissionRule::allow_tool("Read"),
    PermissionScope::Global,
);

// Or during spawn
let handle = runtime.spawn_with_local_rules(
    session,
    vec![
        PermissionRule::allow_tool("Read"),
        PermissionRule::allow_prefix("Bash", "cd "),
    ],
    |internals| agent.run(internals),
).await;
```

#### Handling Permission Requests

```rust
// In your output handler
match chunk {
    OutputChunk::PermissionRequest { tool_name, action, input, details } => {
        // Show UI to user
        println!("Tool '{}' wants to: {}", tool_name, action);
        println!("Input: {}", input);

        // Get user decision (true = allow, false = deny)
        let allowed = show_permission_dialog(&action);
        let remember = ask_if_remember();

        // Send response
        handle.send_permission_response(tool_name, allowed, remember).await?;
    }
    _ => {}
}
```

### Hooks Module

Intercept and modify agent behavior at key points.

#### Hook Events

```rust
use shadow_agent_sdk::hooks::{HookRegistry, HookEvent, HookContext, HookResult};

let mut hooks = HookRegistry::new();

// PreToolUse - Before tool executes (can block, allow, or modify)
hooks.add(HookEvent::PreToolUse, |ctx: &mut HookContext| {
    println!("About to use tool: {}", ctx.tool_name.as_ref().unwrap());
    HookResult::none()  // Continue normal flow
})?;

// PostToolUse - After successful execution
hooks.add(HookEvent::PostToolUse, |ctx: &mut HookContext| {
    println!("Tool completed: {:?}", ctx.tool_result);
    HookResult::none()
})?;

// PostToolUseFailure - After tool fails
hooks.add(HookEvent::PostToolUseFailure, |ctx: &mut HookContext| {
    println!("Tool failed: {:?}", ctx.error);
    HookResult::none()
})?;

// UserPromptSubmit - When user sends a message
hooks.add(HookEvent::UserPromptSubmit, |ctx: &mut HookContext| {
    println!("User said: {:?}", ctx.user_prompt);
    HookResult::none()
})?;
```

#### Pattern-Based Hooks

```rust
// Only match specific tools
hooks.add_with_pattern(HookEvent::PreToolUse, "Bash", |ctx| {
    // Only runs for Bash tool
    let input = ctx.tool_input.as_ref().unwrap();
    if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
        if cmd.contains("rm -rf /") {
            return HookResult::deny("Dangerous command blocked");
        }
    }
    HookResult::none()
})?;

// Regex patterns
hooks.add_with_pattern(HookEvent::PreToolUse, "Write|Edit", |ctx| {
    // Runs for Write or Edit tools
    HookResult::none()
})?;
```

#### Hook Results

```rust
// Allow - Skip permission check, execute immediately
HookResult::allow()

// Deny - Block execution with reason
HookResult::deny("Not allowed")

// Ask - Use normal permission flow
HookResult::ask()

// None - Continue normal flow (check permissions)
HookResult::none()
```

#### Modifying Tool Input

```rust
hooks.add(HookEvent::PreToolUse, |ctx: &mut HookContext| {
    if let Some(input) = ctx.tool_input.as_mut() {
        // Modify the input before execution
        if let Some(obj) = input.as_object_mut() {
            obj.insert("modified".to_string(), json!(true));
        }
    }
    HookResult::none()
})?;
```

### LLM Module

Interface with Anthropic's Claude API.

#### AnthropicProvider

```rust
use shadow_agent_sdk::llm::AnthropicProvider;

// From environment variable (ANTHROPIC_API_KEY)
let llm = AnthropicProvider::from_env()?;

// With explicit API key
let llm = AnthropicProvider::new("sk-ant-...")?;

// With custom model
let llm = AnthropicProvider::from_env()?
    .with_model("claude-sonnet-4-5-20250929");

// With custom max tokens
let llm = AnthropicProvider::from_env()?
    .with_max_tokens(8192);

// Wrap in Arc for sharing
let llm = Arc::new(llm);
```

#### Dynamic Authentication

For JWT tokens, rotating keys, or proxy servers:

```rust
use shadow_agent_sdk::llm::{AnthropicProvider, AuthConfig, AuthProvider};
use async_trait::async_trait;

struct MyAuthProvider {
    // Your auth state
}

#[async_trait]
impl AuthProvider for MyAuthProvider {
    async fn get_auth(&self) -> anyhow::Result<AuthConfig> {
        // Fetch/refresh credentials
        let token = refresh_jwt_token().await?;
        Ok(AuthConfig::new(token))
    }
}

let llm = AnthropicProvider::with_auth_provider(MyAuthProvider { ... })?;
```

#### Extended Thinking

```rust
use shadow_agent_sdk::llm::ThinkingConfig;

let config = AgentConfig::new("You are a thoughtful assistant.")
    .with_thinking(16000);  // Budget in tokens

// Or with config object
let config = AgentConfig::new("...")
    .with_thinking_config(ThinkingConfig::enabled(32000));
```

#### Message Types

```rust
use shadow_agent_sdk::llm::{Message, MessageContent, ContentBlock};

// Simple text message
let msg = Message::user("Hello");
let msg = Message::assistant("Hi there!");

// With content blocks
let msg = Message::user_with_blocks(vec![
    ContentBlock::text("Analyze this"),
]);

// Access content
match &message.content {
    MessageContent::Text(text) => println!("{}", text),
    MessageContent::Blocks(blocks) => {
        for block in blocks {
            match block {
                ContentBlock::Text { text } => println!("{}", text),
                ContentBlock::ToolUse { id, name, input } => {
                    println!("Tool: {} ({})", name, id);
                }
                ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                    println!("Result for {}: {:?}", tool_use_id, content);
                }
                ContentBlock::Thinking { thinking, .. } => {
                    println!("Thinking: {}", thinking);
                }
                _ => {}
            }
        }
    }
}
```

### Helpers Module

Utilities for common agent patterns.

#### Context Injection

Modify messages before each LLM call:

```rust
use shadow_agent_sdk::helpers::{ContextInjection, InjectionChain, FnInjection};

// Function-based injection
let injection = FnInjection::new("add_timestamp", |internals, messages| {
    let timestamp = chrono::Utc::now().to_rfc3339();
    shadow_agent_sdk::helpers::inject_system_reminder(
        messages,
        &format!("Current time: {}", timestamp),
    );
});

// Chain multiple injections
let mut chain = InjectionChain::new();
chain.add(injection);
chain.add_fn("add_context", |internals, messages| {
    // Access session state
    let turn = internals.context.current_turn;
    shadow_agent_sdk::helpers::inject_system_reminder(
        messages,
        &format!("This is turn {}", turn),
    );
});

let config = AgentConfig::new("...")
    .with_injection_chain(chain);
```

#### Helper Functions

```rust
use shadow_agent_sdk::helpers::{
    prepend_to_first_user_message,
    append_to_last_message,
    inject_system_reminder,
};

// Add text to first user message
prepend_to_first_user_message(&mut messages, "IMPORTANT: ");

// Add text to last message
append_to_last_message(&mut messages, "\n\nRemember to be concise.");

// Add system reminder (creates new assistant message if needed)
inject_system_reminder(&mut messages, "The user prefers detailed explanations.");
```

#### Debugger

Log all API calls and tool executions:

```rust
let config = AgentConfig::new("...")
    .with_debug(true);

// Logs are written to: sessions/{session_id}/debugger/
// - api_request_{n}.json
// - api_response_{n}.json
// - tool_call_{n}.json
// - tool_result_{n}.json
```

---

## Building a Frontend Integration

### Tauri Example Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Tauri Frontend (JS/TS)                     │
│  - Chat UI                                                      │
│  - Permission dialogs                                           │
│  - Session browser                                              │
└─────────────────────┬───────────────────────────────────────────┘
                      │ IPC Commands
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Tauri Backend (Rust)                       │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    AppState                                │  │
│  │  - runtime: AgentRuntime                                   │  │
│  │  - llm: Arc<AnthropicProvider>                            │  │
│  │  - tools: Arc<ToolRegistry>                                │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│  Commands:                                                       │
│  - create_agent(session_id, agent_type)                         │
│  - send_message(session_id, message)                            │
│  - send_permission(session_id, tool, allowed, remember)         │
│  - list_sessions(top_level_only)                                │
│  - get_history(session_id)                                      │
│  - shutdown_agent(session_id)                                   │
└─────────────────────────────────────────────────────────────────┘
```

### Backend State

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub runtime: AgentRuntime,
    pub llm: Arc<AnthropicProvider>,
    pub tools: Arc<ToolRegistry>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let llm = Arc::new(AnthropicProvider::from_env()?);

        let mut tools = ToolRegistry::new();
        tools.register(ReadTool::new()?);
        tools.register(WriteTool::new()?);
        tools.register(BashTool::new()?);
        let tools = Arc::new(tools);

        let runtime = AgentRuntime::with_global_rules(vec![
            PermissionRule::allow_tool("Read"),
        ]);

        Ok(Self { runtime, llm, tools })
    }
}
```

### Tauri Commands

```rust
#[tauri::command]
async fn create_agent(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    session_id: String,
    agent_type: String,
    name: String,
    system_prompt: String,
) -> Result<(), String> {
    let state = state.lock().await;

    let session = AgentSession::new(&session_id, &agent_type, &name, "")
        .map_err(|e| e.to_string())?;

    let config = AgentConfig::new(&system_prompt)
        .with_tools(state.tools.clone())
        .with_streaming(true);

    let agent = StandardAgent::new(config, state.llm.clone());

    state.runtime
        .spawn(session, |internals| agent.run(internals))
        .await;

    Ok(())
}

#[tauri::command]
async fn send_message(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    window: tauri::Window,
    session_id: String,
    message: String,
) -> Result<(), String> {
    let state = state.lock().await;

    let handle = state.runtime.get(&session_id).await
        .ok_or("Agent not found")?;

    // Subscribe BEFORE sending
    let mut rx = handle.subscribe();

    handle.send_input(&message).await
        .map_err(|e| e.to_string())?;

    // Spawn task to forward output to frontend
    let window_clone = window.clone();
    tokio::spawn(async move {
        while let Ok(chunk) = rx.recv().await {
            let event_name = match &chunk {
                OutputChunk::TextDelta(_) => "text-delta",
                OutputChunk::ToolStart { .. } => "tool-start",
                OutputChunk::ToolEnd { .. } => "tool-end",
                OutputChunk::PermissionRequest { .. } => "permission-request",
                OutputChunk::Error(_) => "error",
                OutputChunk::Done => "done",
                _ => continue,
            };

            let _ = window_clone.emit(event_name, &chunk);

            if matches!(chunk, OutputChunk::Done | OutputChunk::Error(_)) {
                break;
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn send_permission(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    session_id: String,
    tool_name: String,
    allowed: bool,
    remember: bool,
) -> Result<(), String> {
    let state = state.lock().await;

    let handle = state.runtime.get(&session_id).await
        .ok_or("Agent not found")?;

    handle.send_permission_response(tool_name, allowed, remember).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_sessions(top_level_only: bool) -> Result<Vec<SessionInfo>, String> {
    let sessions = AgentSession::list_with_metadata(top_level_only)
        .map_err(|e| e.to_string())?;

    Ok(sessions.into_iter().map(|(id, meta)| SessionInfo {
        session_id: id,
        agent_type: meta.agent_type,
        name: meta.name,
        description: meta.description,
        created_at: meta.created_at.to_rfc3339(),
        updated_at: meta.updated_at.to_rfc3339(),
        is_subagent: meta.is_subagent(),
    }).collect())
}

#[tauri::command]
async fn get_history(session_id: String) -> Result<Vec<MessageInfo>, String> {
    let history = AgentSession::get_history(&session_id)
        .map_err(|e| e.to_string())?;

    Ok(history.into_iter().map(|msg| MessageInfo {
        role: msg.role,
        content: format_content(&msg.content),
    }).collect())
}
```

### Frontend (TypeScript)

```typescript
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';

// Create agent
await invoke('create_agent', {
  sessionId: 'chat-1',
  agentType: 'assistant',
  name: 'My Assistant',
  systemPrompt: 'You are a helpful assistant.',
});

// Listen for output
await listen('text-delta', (event) => {
  appendToChat(event.payload);
});

await listen('permission-request', (event) => {
  const { tool_name, action, input } = event.payload;
  showPermissionDialog(tool_name, action, input, async (allowed, remember) => {
    await invoke('send_permission', {
      sessionId: 'chat-1',
      toolName: tool_name,
      allowed,
      remember,
    });
  });
});

await listen('done', () => {
  setLoading(false);
});

// Send message
await invoke('send_message', {
  sessionId: 'chat-1',
  message: 'Hello!',
});

// List sessions
const sessions = await invoke('list_sessions', { topLevelOnly: true });

// Get history
const history = await invoke('get_history', { sessionId: 'chat-1' });
```

---

## Examples

### Test Agent (`examples/test_agent/`)

Basic agent with common tools:

```bash
cargo run --example test_agent
```

### Integration Test (`examples/integration_test/`)

Demonstrates subagent spawning:

```bash
cargo run --example integration_test
```

### Session Browser (`examples/session_browser/`)

Lists sessions and shows history:

```bash
cargo run --example session_browser
```

---

## API Reference

### OutputChunk Variants

| Variant | Description | Fields |
|---------|-------------|--------|
| `TextDelta(String)` | Streaming text token | Text content |
| `TextComplete(String)` | Full text response | Complete text |
| `ThinkingDelta(String)` | Streaming thinking token | Thinking content |
| `ThinkingComplete(String)` | Full thinking | Complete thinking |
| `ToolStart` | Tool execution starting | `id`, `name`, `input` |
| `ToolProgress` | Tool progress update | `id`, `output` |
| `ToolEnd` | Tool execution complete | `id`, `result` |
| `PermissionRequest` | Permission needed | `tool_name`, `action`, `input`, `details` |
| `SubAgentSpawned` | Subagent created | `session_id`, `agent_type` |
| `SubAgentOutput` | Subagent output | `session_id`, `chunk` |
| `SubAgentComplete` | Subagent done | `session_id`, `result` |
| `StateChange(AgentState)` | State transition | New state |
| `Status(String)` | Status message | Message |
| `Error(String)` | Error occurred | Error message |
| `Done` | Agent finished | None |

### InputMessage Variants

| Variant | Description |
|---------|-------------|
| `UserInput(String)` | User prompt |
| `ToolResult { tool_use_id, result }` | Async tool completion |
| `PermissionResponse { tool_name, allowed, remember }` | Permission decision |
| `SubAgentComplete { session_id, result }` | Subagent finished |
| `Interrupt` | Cancel current operation |
| `Shutdown` | Stop agent |

### AgentConfig Builder Methods

| Method | Description |
|--------|-------------|
| `with_tools(Arc<ToolRegistry>)` | Set available tools |
| `with_streaming(bool)` | Enable/disable streaming |
| `with_debug(bool)` | Enable debug logging |
| `with_thinking(budget)` | Enable extended thinking |
| `with_hooks(Arc<HookRegistry>)` | Set behavior hooks |
| `with_max_tool_iterations(n)` | Limit tool call loops |
| `with_auto_save(bool)` | Auto-save session |
| `with_injection_chain(chain)` | Set context injections |

### Session Methods

| Method | Description |
|--------|-------------|
| `AgentSession::new(...)` | Create root session |
| `AgentSession::new_subagent(...)` | Create linked subagent |
| `AgentSession::load(id)` | Load from disk |
| `AgentSession::list_all()` | List all session IDs |
| `AgentSession::list_top_level()` | List non-subagents |
| `AgentSession::list_with_metadata(top_level)` | List with metadata |
| `AgentSession::get_history(id)` | Get messages only |
| `AgentSession::get_metadata(id)` | Get metadata only |
| `AgentSession::exists(id)` | Check existence |

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ANTHROPIC_API_KEY` | API key for Anthropic | Required |
| `RUST_LOG` | Log level (debug, info, warn, error) | info |

---

## License

MIT
