# Framework Implementation Plan

## Overview

Incremental implementation with human-in-the-loop review at each phase. Each phase ends with a testable state.

## Phase Summary

| Phase | Focus | Deliverable | Test | Status |
|-------|-------|-------------|------|--------|
| 1 | Core Types | Basic types compiling | Unit tests | ✅ Complete |
| 2 | Session Manager | Session CRUD working | Create/load sessions | ✅ Complete (17 tests) |
| 3 | Channels & Handle | Message passing working | Send/receive messages | ✅ Complete (23 tests) |
| 4 | Runtime | Agent spawning working | Spawn agent task | ✅ Complete (8 tests) |
| 5 | Console Renderer | CLI working | Interactive input/output | Pending |
| 6 | Integration Test | Two agents running | Multi-agent test | Pending |
| 7 | Permission System | Rules working | Permission checks | Pending |
| 8 | Tool System | Tools with context | Execute tools | Pending |
| 9 | Full Integration | Everything together | Full agent test | Pending |

---

## Phase 1: Core Types

**Goal:** Define all the core types that everything else depends on.

**Files to Create:**
```
src/core/
├── mod.rs
├── context.rs      # AgentContext
├── output.rs       # OutputChunk, InputMessage
├── state.rs        # AgentState
└── error.rs        # FrameworkError
```

**Implementation:**

1. `src/core/error.rs`
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FrameworkError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Agent not running: {0}")]
    AgentNotRunning(String),

    #[error("Channel closed")]
    ChannelClosed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, FrameworkError>;
```

2. `src/core/state.rs`
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    Idle,
    Processing,
    WaitingForPermission,
    ExecutingTool(String),
    WaitingForSubAgent(String),
    Done,
    Error(String),
}
```

3. `src/core/output.rs`
```rust
// InputMessage and OutputChunk enums
```

4. `src/core/context.rs`
```rust
// AgentContext struct
```

**Test:** `cargo build` - everything compiles

**Review Point:** Check types look correct before proceeding.

---

## Phase 2: Session Manager

**Goal:** Replace `Conversation` with `AgentSession`. CRUD operations working.

**Files to Create:**
```
src/session/
├── mod.rs
├── session.rs      # AgentSession struct
├── storage.rs      # File I/O helpers
└── metadata.rs     # SessionMetadata
```

**Implementation:**

1. Create `AgentSession` with:
   - `new()`, `new_subagent()`
   - `add_message()`, `get_history()`
   - `save()`, `load()`
   - Metadata methods

2. Storage in `sessions/<session_id>/` folder

**Test Script:**
```rust
// test_session.rs
#[tokio::test]
async fn test_session_crud() {
    // Create session
    let mut session = AgentSession::new("test-agent");
    assert!(!session.session_id().is_empty());

    // Add messages
    session.add_message(Message::user("Hello"));
    session.add_message(Message::assistant("Hi there"));

    // Save
    session.save().unwrap();

    // Load
    let loaded = AgentSession::load(session.session_id()).unwrap();
    assert_eq!(loaded.get_history().len(), 2);

    // Create subagent
    let sub = AgentSession::new_subagent(
        "sub-agent",
        session.session_id(),
        "tool_123",
    );
    assert_eq!(sub.parent_session_id(), Some(session.session_id()));
}
```

**Review Point:** Verify session storage format, metadata structure.

---

## Phase 3: Channels & AgentHandle

**Goal:** Set up the communication channels and AgentHandle interface.

**Files to Create:**
```
src/runtime/
├── mod.rs
├── channels.rs     # Channel type definitions
├── handle.rs       # AgentHandle
└── internals.rs    # AgentInternals
```

**Implementation:**

1. `channels.rs` - Define channel types:
```rust
pub type InputSender = mpsc::Sender<InputMessage>;
pub type InputReceiver = mpsc::Receiver<InputMessage>;
pub type OutputSender = broadcast::Sender<OutputChunk>;
pub type OutputReceiver = broadcast::Receiver<OutputChunk>;
```

2. `handle.rs` - AgentHandle:
   - Constructor from channels
   - `send_input()`, `subscribe()`, `state()`
   - `interrupt()`, `shutdown()`

3. `internals.rs` - AgentInternals:
   - `receive()`, `send()`, `set_state()`

**Test Script:**
```rust
#[tokio::test]
async fn test_channels() {
    let (input_tx, mut input_rx) = mpsc::channel(32);
    let (output_tx, _) = broadcast::channel(32);

    // Send input
    input_tx.send(InputMessage::UserInput("Hello".into())).await.unwrap();

    // Receive input
    let msg = input_rx.recv().await.unwrap();
    assert!(matches!(msg, InputMessage::UserInput(_)));

    // Broadcast output
    let mut rx1 = output_tx.subscribe();
    let mut rx2 = output_tx.subscribe();

    output_tx.send(OutputChunk::TextDelta("Hi".into())).unwrap();

    assert!(matches!(rx1.recv().await.unwrap(), OutputChunk::TextDelta(_)));
    assert!(matches!(rx2.recv().await.unwrap(), OutputChunk::TextDelta(_)));
}
```

**Review Point:** Verify channel semantics, handle interface.

---

## Phase 4: AgentRuntime

**Goal:** Runtime can spawn agent tasks and return handles.

**Files to Create/Update:**
```
src/runtime/
└── runtime.rs      # AgentRuntime
```

**Implementation:**

```rust
impl AgentRuntime {
    pub fn new() -> Self;

    pub fn spawn<F, Fut>(&self, session: AgentSession, agent_fn: F) -> AgentHandle
    where
        F: FnOnce(AgentInternals) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static;

    pub fn get(&self, session_id: &str) -> Option<AgentHandle>;
    pub fn shutdown(&self, session_id: &str) -> Result<()>;
}
```

**Test Script:**
```rust
#[tokio::test]
async fn test_runtime_spawn() {
    let runtime = AgentRuntime::new();
    let session = AgentSession::new("test");

    // Spawn a simple agent that echoes input
    let handle = runtime.spawn(session, |mut internals| async move {
        loop {
            match internals.receive().await? {
                InputMessage::UserInput(text) => {
                    internals.send(OutputChunk::TextDelta(format!("Echo: {}", text))).await?;
                    internals.send(OutputChunk::Done).await?;
                }
                InputMessage::Shutdown => break,
                _ => {}
            }
        }
        Ok(())
    });

    // Test interaction
    let mut rx = handle.subscribe();
    handle.send_input("Hello".into()).unwrap();

    let chunk = rx.recv().await.unwrap();
    assert!(matches!(chunk, OutputChunk::TextDelta(s) if s.contains("Echo")));

    handle.shutdown().unwrap();
}
```

**Review Point:** Verify spawning works, handles are usable.

---

## Phase 5: Console Renderer

**Goal:** CLI that subscribes to an agent and handles I/O.

**Files to Create:**
```
src/cli/
├── mod.rs
└── renderer.rs     # ConsoleRenderer
```

**Implementation:**

```rust
pub struct ConsoleRenderer {
    handle: AgentHandle,
}

impl ConsoleRenderer {
    pub fn new(handle: AgentHandle) -> Self;

    /// Main loop - reads input, sends to agent, renders output
    pub async fn run(&self) -> Result<()> {
        // Spawn output renderer task
        let rx = self.handle.subscribe();
        let render_task = tokio::spawn(async move {
            Self::render_loop(rx).await
        });

        // Input loop
        loop {
            let input = self.read_input()?;
            if input == "exit" {
                self.handle.shutdown()?;
                break;
            }
            self.handle.send_input(input)?;

            // Wait for Done chunk
            // ...
        }

        render_task.await??;
        Ok(())
    }

    async fn render_loop(mut rx: OutputReceiver) {
        while let Ok(chunk) = rx.recv().await {
            match chunk {
                OutputChunk::TextDelta(text) => print!("{}", text),
                OutputChunk::Done => println!(),
                // ... handle other chunks
            }
        }
    }
}
```

**Test:** Manual testing with a simple echo agent.

```rust
// In main.rs temporarily
#[tokio::main]
async fn main() -> Result<()> {
    let runtime = AgentRuntime::new();
    let session = AgentSession::new("echo-test");

    let handle = runtime.spawn(session, |mut internals| async move {
        loop {
            match internals.receive().await? {
                InputMessage::UserInput(text) => {
                    internals.send(OutputChunk::TextDelta(format!("You said: {}\n", text))).await?;
                    internals.send(OutputChunk::Done).await?;
                }
                InputMessage::Shutdown => break,
                _ => {}
            }
        }
        Ok(())
    });

    let console = ConsoleRenderer::new(handle);
    console.run().await?;

    Ok(())
}
```

Run: `cargo run` and interact with the echo agent.

**Review Point:** Verify console I/O works correctly.

---

## Phase 6: Integration Test - Two Agents

**Goal:** Run two agents simultaneously, verify isolation.

**Test Script:**
```rust
#[tokio::test]
async fn test_two_agents() {
    let runtime = AgentRuntime::new();

    // Agent 1: Adds "A:" prefix
    let session1 = AgentSession::new("agent-a");
    let handle1 = runtime.spawn(session1, |mut internals| async move {
        loop {
            match internals.receive().await? {
                InputMessage::UserInput(text) => {
                    internals.send(OutputChunk::TextDelta(format!("A: {}", text))).await?;
                    internals.send(OutputChunk::Done).await?;
                }
                InputMessage::Shutdown => break,
                _ => {}
            }
        }
        Ok(())
    });

    // Agent 2: Adds "B:" prefix
    let session2 = AgentSession::new("agent-b");
    let handle2 = runtime.spawn(session2, |mut internals| async move {
        loop {
            match internals.receive().await? {
                InputMessage::UserInput(text) => {
                    internals.send(OutputChunk::TextDelta(format!("B: {}", text))).await?;
                    internals.send(OutputChunk::Done).await?;
                }
                InputMessage::Shutdown => break,
                _ => {}
            }
        }
        Ok(())
    });

    // Subscribe to both
    let mut rx1 = handle1.subscribe();
    let mut rx2 = handle2.subscribe();

    // Send to both
    handle1.send_input("Hello".into()).unwrap();
    handle2.send_input("World".into()).unwrap();

    // Verify isolation
    let chunk1 = rx1.recv().await.unwrap();
    let chunk2 = rx2.recv().await.unwrap();

    assert!(matches!(&chunk1, OutputChunk::TextDelta(s) if s.starts_with("A:")));
    assert!(matches!(&chunk2, OutputChunk::TextDelta(s) if s.starts_with("B:")));

    // Verify different session IDs
    assert_ne!(handle1.session_id(), handle2.session_id());

    // Cleanup
    handle1.shutdown().unwrap();
    handle2.shutdown().unwrap();
}
```

**Review Point:** Verify multi-agent isolation works.

---

## Phase 7: Permission System

**Goal:** Rule-based permission checking.

**Files to Create:**
```
src/permissions/
├── mod.rs
├── rule.rs         # PermissionRule, RuleMatcher, RuleDecision
├── manager.rs      # PermissionManager
└── evaluator.rs    # Rule evaluation logic
```

**Implementation:**

1. `rule.rs` - Rule types
2. `manager.rs` - Add/check rules
3. `evaluator.rs` - Match and evaluate

**Test Script:**
```rust
#[test]
fn test_permission_rules() {
    let mut manager = PermissionManager::new();

    // Global rule: allow all read operations
    manager.add_rule(PermissionRule::allow_category("read"));

    // Global rule: ask for bash
    manager.add_rule(PermissionRule::ask_for_tool("Bash"));

    // Global rule: allow specific bash commands
    manager.add_rule(
        PermissionRule::allow_bash_prefix(vec!["echo", "ls", "pwd"])
            .with_priority(10)  // Higher priority
    );

    let context = AgentContext::new_test();

    // Read tool -> Allow
    assert_eq!(
        manager.check("Read", &json!({"file": "test.txt"}), &context),
        RuleDecision::Allow
    );

    // Bash with echo -> Allow (higher priority rule)
    assert_eq!(
        manager.check("Bash", &json!({"command": "echo hello"}), &context),
        RuleDecision::Allow
    );

    // Bash with rm -> AskUser (default bash rule)
    assert_eq!(
        manager.check("Bash", &json!({"command": "rm -rf /"}), &context),
        RuleDecision::AskUser
    );

    // Unknown tool -> AskUser (default)
    assert_eq!(
        manager.check("Unknown", &json!({}), &context),
        RuleDecision::AskUser
    );
}
```

**Review Point:** Verify rule matching logic.

---

## Phase 8: Tool System

**Goal:** Updated Tool trait with AgentContext, registry working.

**Files to Update:**
```
src/tools/
├── mod.rs
├── tool.rs         # Updated Tool trait
├── registry.rs     # Updated ToolRegistry
└── builtin/        # Update all tools
```

**Implementation:**

1. Update `Tool` trait signature:
```rust
async fn execute(&self, input: &Value, context: &AgentContext) -> Result<ToolResult>;
```

2. Update all built-in tools to accept context

3. Update `ToolRegistry::execute()` to pass context

**Test Script:**
```rust
#[tokio::test]
async fn test_tool_with_context() {
    let mut registry = ToolRegistry::new();
    registry.register(ReadTool::new().unwrap());

    let context = AgentContext {
        session_id: "test-session".into(),
        agent_type: "test".into(),
        parent_session_id: None,
        parent_tool_use_id: None,
        current_turn: 1,
        current_tool_use_id: Some("tool_123".into()),
        metadata: HashMap::new(),
    };

    // Execute tool with context
    let result = registry.execute(
        "Read",
        &json!({"file_path": "/tmp/test.txt"}),
        &context,
    ).await;

    // Tool has access to context.session_id, context.current_tool_use_id, etc.
}
```

**Review Point:** Verify tools receive context correctly.

---

## Phase 9: Full Integration

**Goal:** Put everything together with a real LLM-powered agent.

**Implementation:**

1. Create example agent in `examples/coder_agent.rs`
2. Wire up all components
3. Test with actual LLM calls

**Test:** Interactive session with full agent.

```bash
cargo run --example coder_agent
```

**Review Point:** Full system test.

---

## File Checklist

### Phase 1: Core Types ✅ COMPLETE
- [x] `src/core/mod.rs`
- [x] `src/core/error.rs`
- [x] `src/core/state.rs`
- [x] `src/core/output.rs`
- [x] `src/core/context.rs` (includes name and description fields)

### Phase 2: Session Manager ✅ COMPLETE
- [x] `src/session/mod.rs`
- [x] `src/session/session.rs` (AgentSession with full CRUD)
- [x] `src/session/storage.rs` (file I/O helpers)
- [x] `src/session/metadata.rs` (SessionMetadata with lineage)

### Phase 3: Channels & Handle ✅ COMPLETE
- [x] `src/runtime/mod.rs`
- [x] `src/runtime/channels.rs` (channel type definitions, create helpers)
- [x] `src/runtime/handle.rs` (AgentHandle - external interface)
- [x] `src/runtime/internals.rs` (AgentInternals - internal state)

### Phase 4: Runtime ✅ COMPLETE
- [x] `src/runtime/runtime.rs` (AgentRuntime - spawn, registry, lifecycle)

### Phase 5: Console Renderer
- [ ] `src/cli/mod.rs`
- [ ] `src/cli/renderer.rs`

### Phase 6: Integration Test
- [ ] `tests/multi_agent.rs`

### Phase 7: Permission System
- [ ] `src/permissions/mod.rs`
- [ ] `src/permissions/rule.rs`
- [ ] `src/permissions/manager.rs`
- [ ] `src/permissions/evaluator.rs`

### Phase 8: Tool System
- [ ] Update `src/tools/tool.rs`
- [ ] Update `src/tools/registry.rs`
- [ ] Update all tools in `src/tools/builtin/`

### Phase 9: Full Integration
- [ ] `examples/coder_agent.rs`
- [ ] Update `src/main.rs`

---

## Dependencies to Add

```toml
[dependencies]
# Existing...

# New
thiserror = "1.0"      # Error handling
tokio-stream = "0.1"   # Async streams
```

---

## Timeline Estimate

| Phase | Estimated Effort |
|-------|------------------|
| 1 | 30 min |
| 2 | 1 hour |
| 3 | 1 hour |
| 4 | 1 hour |
| 5 | 1 hour |
| 6 | 30 min |
| 7 | 1-2 hours |
| 8 | 1-2 hours |
| 9 | 1-2 hours |

Total: ~8-10 hours of implementation + review time

---

## Next Steps

1. Review this plan
2. Approve or request changes
3. Start Phase 1 implementation
4. Review after each phase before proceeding
