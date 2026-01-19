
# Agent Framework Architecture

## Overview

This document describes the architecture for an extensible agent framework. The goal is to standardize common components (I/O, sessions, tool execution, permissions, runtime) while giving programmers full control over agent behavior.

## Design Philosophy

**What's Standardized:**
- AgentHandle (input/output interface with streaming)
- AgentSession (storage, history, metadata)
- AgentRuntime (spawning agents, lifecycle management)
- Tool execution (trait, context passing)
- Permission rules (evaluation system)

**What's Programmer-Controlled:**
- Agent loop logic (when to call LLM, how to handle responses)
- Subagent spawning decisions (blocking vs async)
- Which tools to include per agent
- Custom behaviors, hooks, skills
- Prompt engineering
- Model selection per agent

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PROGRAMMER CONTROLS                               │
│                                                                      │
│   - Agent loop logic (when to call LLM, how to handle responses)    │
│   - Subagent spawning decisions (blocking vs async)                 │
│   - Which tools to include                                          │
│   - Custom behaviors, hooks, skills                                  │
│   - Prompt engineering                                               │
│   - Model selection per agent                                        │
└─────────────────────────────────────────────────────────────────────┘
                              │ uses
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    FRAMEWORK PROVIDES                                │
│                                                                      │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │
│   │ AgentHandle │  │ AgentSession│  │ ToolRegistry│                │
│   │ (I/O)       │  │ (storage)   │  │ (execution) │                │
│   └─────────────┘  └─────────────┘  └─────────────┘                │
│                                                                      │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │
│   │ Permission  │  │ AgentRuntime│  │ AgentContext│                │
│   │ Manager     │  │ (lifecycle) │  │ (tool state)│                │
│   └─────────────┘  └─────────────┘  └─────────────┘                │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. AgentSession

Manages conversation history, metadata, and storage. Replaces the old `Conversation` struct.

```rust
pub struct AgentSession {
    // Identity
    session_id: String,
    agent_type: String,

    // Lineage (for subagents)
    parent_session_id: Option<String>,
    parent_tool_use_id: Option<String>,
    child_session_ids: Vec<String>,

    // LLM settings
    model: String,
    provider: String,

    // Timestamps
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,

    // History
    messages: Vec<Message>,

    // Extensible metadata
    metadata: HashMap<String, Value>,
}

impl AgentSession {
    // Creation
    pub fn new(agent_type: &str) -> Self;
    pub fn new_subagent(
        agent_type: &str,
        parent_session_id: &str,
        parent_tool_use_id: &str,
    ) -> Self;

    // History management
    pub fn add_message(&mut self, msg: Message);
    pub fn get_history(&self) -> &[Message];
    pub fn get_history_mut(&mut self) -> &mut Vec<Message>;

    // Metadata
    pub fn set_metadata(&mut self, key: &str, value: Value);
    pub fn get_metadata(&self, key: &str) -> Option<&Value>;

    // Subagent tracking
    pub fn add_child_session(&mut self, child_session_id: String);
    pub fn get_child_sessions(&self) -> &[String];

    // Persistence
    pub fn save(&self) -> Result<()>;
    pub fn load(session_id: &str) -> Result<Self>;
    pub fn list_all() -> Result<Vec<String>>;
}
```

**Storage Structure:**
```
sessions/
└── <session_id>/
    ├── metadata.json     # Session metadata
    └── history.jsonl     # Message history
```

**metadata.json:**
```json
{
  "session_id": "abc123",
  "agent_type": "coder",
  "parent_session_id": null,
  "parent_tool_use_id": null,
  "child_session_ids": ["def456", "ghi789"],
  "model": "claude-sonnet-4-5-20250929",
  "provider": "anthropic",
  "created_at": "2026-01-14T10:30:00Z",
  "updated_at": "2026-01-14T10:45:00Z",
  "metadata": {
    "custom_key": "custom_value"
  }
}
```

---

### 2. AgentHandle

The standardized interface for communicating with a running agent. External code (console, parent agent) uses this to send input and receive streaming output.

```rust
pub struct AgentHandle {
    session_id: String,
    input_tx: mpsc::Sender<InputMessage>,
    output_tx: broadcast::Sender<OutputChunk>,
    state: Arc<RwLock<AgentState>>,
    session: Arc<RwLock<AgentSession>>,
}

impl AgentHandle {
    // --- Input ---

    /// Send user input to the agent
    pub fn send_input(&self, input: String) -> Result<()>;

    /// Send a tool result back to the agent (for async tool execution)
    pub fn send_tool_result(&self, tool_use_id: String, result: ToolResult) -> Result<()>;

    /// Request graceful interrupt
    pub fn interrupt(&self) -> Result<()>;

    /// Request shutdown
    pub fn shutdown(&self) -> Result<()>;

    // --- Output (Streaming) ---

    /// Get a receiver for output chunks (can have multiple subscribers)
    pub fn subscribe(&self) -> broadcast::Receiver<OutputChunk>;

    /// Async stream of output chunks
    pub async fn stream(&self) -> impl Stream<Item = OutputChunk>;

    // --- State ---

    /// Get current agent state
    pub fn state(&self) -> AgentState;

    /// Get session ID
    pub fn session_id(&self) -> &str;

    /// Get conversation history
    pub fn get_history(&self) -> Vec<Message>;

    /// Get session metadata
    pub fn get_metadata(&self) -> HashMap<String, Value>;

    // --- Lineage ---

    /// Get parent info if this is a subagent
    pub fn parent_info(&self) -> Option<ParentInfo>;

    /// Get list of child session IDs
    pub fn child_sessions(&self) -> Vec<String>;
}
```

**Input Messages:**
```rust
pub enum InputMessage {
    UserInput(String),
    ToolResult { tool_use_id: String, result: ToolResult },
    Interrupt,
    Shutdown,
}
```

**Output Chunks:**
```rust
pub enum OutputChunk {
    // Text streaming
    TextDelta(String),
    TextComplete(String),

    // Thinking streaming
    ThinkingDelta(String),
    ThinkingComplete(String),

    // Tool execution
    ToolStart { id: String, name: String, input: Value },
    ToolProgress { id: String, output: String },
    ToolEnd { id: String, result: ToolResult, is_error: bool },

    // Permission requests
    PermissionRequest { tool_name: String, action: String, details: Option<String> },
    PermissionResponse { tool_name: String, decision: RuleDecision },

    // Subagent events
    SubAgentSpawned { session_id: String, agent_type: String },
    SubAgentOutput { session_id: String, chunk: Box<OutputChunk> },
    SubAgentComplete { session_id: String, result: Option<String> },

    // State changes
    StateChange(AgentState),

    // Completion
    Error(String),
    Done,
}
```

**Agent States:**
```rust
pub enum AgentState {
    Idle,                    // Waiting for input
    Processing,              // Processing input, calling LLM
    WaitingForPermission,    // Waiting for user permission decision
    ExecutingTool(String),   // Executing a tool (tool name)
    WaitingForSubAgent(String),  // Waiting for subagent (session_id)
    Done,                    // Agent completed
    Error(String),           // Agent errored
}
```

---

### 3. AgentContext

Hidden state passed to tools during execution. This is NOT exposed in the tool's JSON schema to the LLM.

```rust
pub struct AgentContext {
    // Identity
    pub session_id: String,
    pub agent_type: String,
    pub name: String,
    pub description: String,

    // Lineage (for subagents)
    pub parent_session_id: Option<String>,
    pub parent_tool_use_id: Option<String>,

    // Current execution state
    pub current_turn: usize,
    pub current_tool_use_id: Option<String>,

    // Extensible metadata (JSON-serializable)
    pub metadata: HashMap<String, Value>,

    // Agent-specific resources (runtime objects)
    // NOT serialized - these exist only at runtime
    pub resources: ResourceMap,
}

impl AgentContext {
    pub fn new(session: &AgentSession) -> Self;

    pub fn with_tool_use_id(&self, tool_use_id: &str) -> Self;

    // JSON metadata (serializable)
    pub fn set_metadata(&mut self, key: &str, value: Value);
    pub fn get_metadata(&self, key: &str) -> Option<&Value>;

    // Runtime resources (any Rust type)
    pub fn insert_resource<T: Send + Sync + 'static>(&mut self, value: T);
    pub fn get_resource<T: Send + Sync + 'static>(&self) -> Option<Arc<T>>;
}

/// Type-safe container for agent-specific resources
/// Allows tools to access agent state like TodoManager, FileWatcher, etc.
pub struct ResourceMap {
    map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ResourceMap {
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T);
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>>;
    pub fn contains<T: Send + Sync + 'static>(&self) -> bool;
    pub fn remove<T: Send + Sync + 'static>(&mut self) -> Option<Arc<T>>;
}
```

---

### 4. Tool Trait

Updated tool trait that receives AgentContext during execution.

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (used in LLM tool calls)
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// Tool category for permission rules
    fn category(&self) -> &str;  // "read", "write", "execute", "network", etc.

    /// JSON schema definition for LLM
    fn definition(&self) -> ToolDefinition;

    /// Human-readable info for permission prompts
    fn get_info(&self, input: &Value) -> ToolInfo;

    /// Execute the tool
    /// - input: The LLM-provided input (matches JSON schema)
    /// - context: Hidden state (session_id, parent info, etc.)
    async fn execute(
        &self,
        input: &Value,
        context: &AgentContext,
    ) -> Result<ToolResult>;

    /// Whether this tool requires permission by default
    fn requires_permission(&self) -> bool {
        true
    }
}
```

**ToolRegistry:**
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self;

    pub fn register(&mut self, tool: impl Tool + 'static);

    pub fn get(&self, name: &str) -> Option<&dyn Tool>;

    pub fn get_definitions(&self) -> Vec<ToolDefinition>;

    pub async fn execute(
        &self,
        name: &str,
        input: &Value,
        context: &AgentContext,
    ) -> Result<ToolResult>;

    pub fn list_tools(&self) -> Vec<&str>;

    pub fn get_category(&self, name: &str) -> Option<&str>;
}
```

---

### 5. Permission System

Rule-based permission system with multiple scopes.

```rust
/// Permission rule scope
pub enum RuleScope {
    Global,              // Applies to all agents, all sessions
    Agent(String),       // Applies to specific agent type
    Session(String),     // Applies to specific session
    User,                // Set by user during session
}

/// What the rule matches
pub enum RuleMatcher {
    /// Match by tool name
    ToolName(String),

    /// Match by tool category
    ToolCategory(String),

    /// Match bash commands by prefix
    CommandPrefix(Vec<String>),

    /// Match by regex pattern
    Regex(String),

    /// Custom matcher function
    Custom(Arc<dyn Fn(&str, &Value) -> bool + Send + Sync>),

    /// Match all tools
    All,
}

/// Decision when rule matches
pub enum RuleDecision {
    Allow,
    Deny,
    AskUser,
}

/// A permission rule
pub struct PermissionRule {
    pub id: String,
    pub scope: RuleScope,
    pub priority: i32,      // Higher priority = evaluated first
    pub matcher: RuleMatcher,
    pub decision: RuleDecision,
    pub description: Option<String>,
}

impl PermissionRule {
    // Builder methods
    pub fn allow_tool(name: &str) -> Self;
    pub fn deny_tool(name: &str) -> Self;
    pub fn ask_for_tool(name: &str) -> Self;
    pub fn allow_category(category: &str) -> Self;
    pub fn allow_bash_prefix(prefixes: Vec<&str>) -> Self;
    // etc.
}

/// Permission manager
pub struct PermissionManager {
    rules: Vec<PermissionRule>,
}

impl PermissionManager {
    pub fn new() -> Self;

    /// Add a rule
    pub fn add_rule(&mut self, rule: PermissionRule);

    /// Add multiple rules
    pub fn add_rules(&mut self, rules: Vec<PermissionRule>);

    /// Check permission for a tool call
    pub fn check(
        &self,
        tool_name: &str,
        input: &Value,
        context: &AgentContext,
    ) -> RuleDecision;

    /// Add a session-scoped rule (set by user during session)
    pub fn add_session_rule(&mut self, session_id: &str, rule: PermissionRule);

    /// Clear session rules
    pub fn clear_session_rules(&mut self, session_id: &str);

    /// Load rules from JSON file
    pub fn load_from_file(path: &str) -> Result<Self>;

    /// Save rules to JSON file
    pub fn save_to_file(&self, path: &str) -> Result<()>;
}
```

**Rule Evaluation Order:**
1. User rules (highest priority, set during session)
2. Session rules
3. Agent rules
4. Global rules

First matching rule wins. If no rule matches, default is `AskUser`.

---

### 6. AgentRuntime

Manages agent lifecycle - spawning, tracking, and shutting down agents.

```rust
pub struct AgentRuntime {
    agents: Arc<RwLock<HashMap<String, AgentHandle>>>,
}

impl AgentRuntime {
    pub fn new() -> Self;

    /// Spawn a new agent task
    ///
    /// The agent_fn receives the internal channels and runs the agent logic.
    /// Returns an AgentHandle for external interaction.
    pub fn spawn<F, Fut>(
        &self,
        session: AgentSession,
        agent_fn: F,
    ) -> AgentHandle
    where
        F: FnOnce(AgentInternals) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static;

    /// Get handle to a running agent
    pub fn get(&self, session_id: &str) -> Option<AgentHandle>;

    /// Check if agent is running
    pub fn is_running(&self, session_id: &str) -> bool;

    /// Shutdown a specific agent
    pub fn shutdown(&self, session_id: &str) -> Result<()>;

    /// Shutdown all agents
    pub fn shutdown_all(&self) -> Result<()>;

    /// List all active session IDs
    pub fn list_active(&self) -> Vec<String>;
}

/// Internal channels/state passed to the agent function
pub struct AgentInternals {
    pub session: AgentSession,
    pub context: AgentContext,
    pub input_rx: mpsc::Receiver<InputMessage>,
    pub output_tx: broadcast::Sender<OutputChunk>,
    pub state: Arc<RwLock<AgentState>>,
}

impl AgentInternals {
    /// Receive next input (blocks until available)
    pub async fn receive(&mut self) -> Result<InputMessage>;

    /// Send output chunk
    pub async fn send(&self, chunk: OutputChunk) -> Result<()>;

    /// Update state
    pub fn set_state(&self, state: AgentState);

    /// Get current state
    pub fn state(&self) -> AgentState;
}
```

---

### 7. ConsoleRenderer

Example subscriber that renders agent output to the terminal.

```rust
pub struct ConsoleRenderer {
    handle: AgentHandle,
    colors: ColorScheme,
}

impl ConsoleRenderer {
    pub fn new(handle: AgentHandle) -> Self;

    /// Run the console renderer (blocking)
    /// Subscribes to agent output and renders to terminal
    pub async fn run(&self) -> Result<()>;

    /// Send user input to the agent
    pub fn send_input(&self, input: String) -> Result<()>;

    /// Handle permission request (prompt user)
    fn handle_permission_request(&self, request: &PermissionRequest) -> RuleDecision;

    /// Render output chunk to terminal
    fn render_chunk(&self, chunk: &OutputChunk);
}
```

---

## Programmer Usage Example

```rust
use framework::{
    AgentRuntime, AgentSession, AgentInternals, AgentContext,
    ToolRegistry, PermissionManager, PermissionRule,
    OutputChunk, InputMessage,
};
use framework::tools::{BashTool, ReadTool, EditTool, WriteTool};

// Programmer writes the agent logic - FULL CONTROL
async fn coder_agent(
    mut internals: AgentInternals,
    tools: ToolRegistry,
    permissions: PermissionManager,
    llm: AnthropicProvider,
) -> Result<()> {
    let system_prompt = include_str!("prompts/coder.txt");

    loop {
        // Wait for input
        internals.set_state(AgentState::Idle);
        let input = match internals.receive().await? {
            InputMessage::UserInput(text) => text,
            InputMessage::Interrupt => {
                internals.send(OutputChunk::Done).await?;
                break;
            }
            InputMessage::Shutdown => break,
            _ => continue,
        };

        // Add to session history
        internals.session.add_message(Message::user(&input));

        // Processing
        internals.set_state(AgentState::Processing);

        // Call LLM - programmer has full control here
        let response = llm.send_with_tools(
            internals.session.get_history().to_vec(),
            Some(system_prompt),
            tools.get_definitions(),
            Some(ToolChoice::auto()),
            Some(ThinkingConfig::enabled(16000)),
        ).await?;

        // Process response blocks
        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    internals.send(OutputChunk::TextDelta(text.clone())).await?;
                    internals.session.add_message(Message::assistant(text));
                }

                ContentBlock::Thinking { thinking, .. } => {
                    internals.send(OutputChunk::ThinkingDelta(thinking.clone())).await?;
                }

                ContentBlock::ToolUse { id, name, input } => {
                    internals.set_state(AgentState::ExecutingTool(name.clone()));

                    // Check permission using standard system
                    let decision = permissions.check(name, input, &internals.context);

                    let should_execute = match decision {
                        RuleDecision::Allow => true,
                        RuleDecision::Deny => false,
                        RuleDecision::AskUser => {
                            internals.set_state(AgentState::WaitingForPermission);
                            internals.send(OutputChunk::PermissionRequest {
                                tool_name: name.clone(),
                                action: tools.get(name).unwrap().get_info(input).action_description,
                                details: None,
                            }).await?;

                            // Wait for permission response
                            match internals.receive().await? {
                                InputMessage::PermissionResponse { allowed, .. } => allowed,
                                _ => false,
                            }
                        }
                    };

                    if should_execute {
                        internals.send(OutputChunk::ToolStart {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        }).await?;

                        // Execute with context
                        let ctx = internals.context.with_tool_use_id(id);
                        let result = tools.execute(name, input, &ctx).await?;

                        internals.send(OutputChunk::ToolEnd {
                            id: id.clone(),
                            result: result.clone(),
                            is_error: result.is_error,
                        }).await?;
                    }
                }

                _ => {}
            }
        }

        // Check if done
        if response.stop_reason == Some(StopReason::EndTurn) {
            internals.send(OutputChunk::Done).await?;
        }

        // Save session
        internals.session.save()?;
    }

    Ok(())
}

// In main.rs
#[tokio::main]
async fn main() -> Result<()> {
    // Create runtime
    let runtime = AgentRuntime::new();

    // Create session
    let session = AgentSession::new("coder");

    // Setup tools (programmer explicitly chooses)
    let mut tools = ToolRegistry::new();
    tools.register(BashTool::new()?);
    tools.register(ReadTool::new()?);
    tools.register(EditTool::new()?);
    tools.register(WriteTool::new()?);

    // Setup permissions
    let mut permissions = PermissionManager::new();
    permissions.add_rule(PermissionRule::allow_category("read"));
    permissions.add_rule(PermissionRule::ask_for_tool("Bash"));
    permissions.add_rule(PermissionRule::ask_for_tool("Write"));
    permissions.add_rule(PermissionRule::ask_for_tool("Edit"));

    // Create LLM client
    let llm = AnthropicProvider::from_env()?;

    // Spawn agent
    let handle = runtime.spawn(session, move |internals| {
        coder_agent(internals, tools, permissions, llm)
    });

    // Create console renderer (subscribes to agent output)
    let console = ConsoleRenderer::new(handle.clone());

    // Run console (handles input/output)
    console.run().await?;

    Ok(())
}
```

---

## Module Structure

```
src/
├── lib.rs                     # Public API exports
├── main.rs                    # CLI entry point (example usage)
│
├── core/                      # Core types
│   ├── mod.rs
│   ├── context.rs            # AgentContext
│   ├── output.rs             # OutputChunk, InputMessage
│   ├── state.rs              # AgentState
│   └── error.rs              # Framework errors
│
├── session/                   # Session management
│   ├── mod.rs
│   ├── session.rs            # AgentSession
│   ├── storage.rs            # File storage helpers
│   └── metadata.rs           # Session metadata types
│
├── runtime/                   # Runtime & handles
│   ├── mod.rs
│   ├── runtime.rs            # AgentRuntime
│   ├── handle.rs             # AgentHandle
│   ├── internals.rs          # AgentInternals
│   └── channels.rs           # Channel type definitions
│
├── permissions/               # Rule-based permissions
│   ├── mod.rs
│   ├── rule.rs               # PermissionRule, RuleMatcher, RuleDecision
│   ├── manager.rs            # PermissionManager
│   └── evaluator.rs          # Rule evaluation logic
│
├── tools/                     # Tool system
│   ├── mod.rs
│   ├── tool.rs               # Tool trait
│   ├── registry.rs           # ToolRegistry
│   ├── result.rs             # ToolResult, ToolInfo
│   └── builtin/              # Built-in tools
│       ├── mod.rs
│       ├── bash.rs
│       ├── read.rs
│       ├── edit.rs
│       ├── write.rs
│       ├── glob.rs
│       ├── grep.rs
│       └── todo.rs
│
├── llm/                       # LLM client
│   ├── mod.rs
│   ├── anthropic.rs          # Anthropic provider
│   └── types.rs              # Message, ContentBlock, etc.
│
├── cli/                       # Console renderer
│   ├── mod.rs
│   └── renderer.rs           # ConsoleRenderer
│
├── debugger/                  # Debug logging (existing)
│   └── mod.rs
│
└── logging.rs                 # Logging setup (existing)
```

---

## Migration Notes

### Files to Keep (mostly unchanged)
- `src/llm/anthropic.rs` - LLM client
- `src/llm/types.rs` - Message types
- `src/debugger/mod.rs` - Debug logging
- `src/logging.rs` - Logging setup

### Files to Refactor
- `src/conversation/` → `src/session/` (rename, add lineage support)
- `src/permissions/` → Update with rule-based system
- `src/tools/` → Update Tool trait for AgentContext
- `src/cli/console.rs` → `src/cli/renderer.rs` (use AgentHandle)

### New Files
- `src/core/` - Core types (context, output, state)
- `src/runtime/` - Runtime, handle, internals
- `src/session/` - Session management

### Files to Remove
- `src/agent/agent_loop.rs` - Logic moves to programmer-written code
- `src/context/` - Merged into session/context

---

## Open Questions

1. **Todo Tracker**: Should this be part of the framework or programmer-controlled?

2. **Debugger Integration**: Should the debugger be part of AgentInternals or separate?

3. **Streaming LLM**: Should we add streaming support to AnthropicProvider for real-time text output?

4. **Error Recovery**: How should the framework handle panics in agent tasks?

5. **Rate Limiting**: Should the framework provide rate limiting utilities?
