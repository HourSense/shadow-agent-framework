# SubAgentManager Implementation

## Overview

The `SubAgentManager` provides a way for parent agents to track and manage their spawned subagents. When an agent spawns subagents, it can:
- Access subagent handles by session ID
- Subscribe to subagent output streams
- Track completed subagents and their results
- Query active/completed subagent status

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Parent Agent                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │              AgentInternals                      │    │
│  │  ┌─────────────────────────────────────────┐    │    │
│  │  │           AgentContext                   │    │    │
│  │  │  ┌───────────────────────────────────┐  │    │    │
│  │  │  │       SubAgentManager             │  │    │    │
│  │  │  │  - active: HashMap<id, Handle>    │  │    │    │
│  │  │  │  - completed: HashMap<id, Info>   │  │    │    │
│  │  │  └───────────────────────────────────┘  │    │    │
│  │  │  ┌───────────────────────────────────┐  │    │    │
│  │  │  │       AgentRuntime (ref)          │  │    │    │
│  │  │  └───────────────────────────────────┘  │    │    │
│  │  └─────────────────────────────────────────┘    │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
           │
           │ spawn_subagent()
           ▼
┌─────────────────────┐   ┌─────────────────────┐
│    Subagent A       │   │    Subagent B       │
│  session: "sub-1"   │   │  session: "sub-2"   │
└─────────────────────┘   └─────────────────────┘
```

## Usage

### Spawning Subagents from Parent

The recommended way to spawn subagents is via `AgentInternals::spawn_subagent()`:

```rust
// Inside parent agent
let handle = internals.spawn_subagent(
    "researcher-1",        // session_id
    "researcher",          // agent_type
    "Research Agent",      // name
    "Researches topics",   // description
    "tool_123",           // tool_use_id (for linking to tool call)
    |sub_internals| async move {
        // Subagent logic here
        loop {
            match sub_internals.receive().await {
                Some(InputMessage::UserInput(query)) => {
                    // Do research...
                    sub_internals.send_text("Research results...");
                    sub_internals.send_done();
                }
                Some(InputMessage::Shutdown) | None => break,
                _ => {}
            }
        }
        Ok(())
    },
).await?;
```

This automatically:
1. Creates the subagent with proper parent linkage
2. Registers the handle with the parent's `SubAgentManager`
3. Sends `OutputChunk::SubAgentSpawned` to subscribers

### Accessing Subagent Handles

```rust
// Get a specific subagent's handle
if let Some(handle) = internals.get_subagent("researcher-1") {
    // Subscribe to its output
    let mut rx = handle.subscribe();

    // Send it input
    handle.send_input("Search for Rust async patterns").await?;

    // Receive output
    while let Ok(chunk) = rx.recv().await {
        match chunk {
            OutputChunk::TextDelta(text) => println!("Subagent says: {}", text),
            OutputChunk::Done => break,
            _ => {}
        }
    }
}

// List all active subagents
let active = internals.active_subagents();
println!("Active subagents: {:?}", active);
```

### Tracking Completed Subagents

```rust
// Mark a subagent as completed (called when subagent finishes)
internals.mark_subagent_completed(
    "researcher-1",
    Some("Found 5 relevant articles".to_string()),
    true,  // success
    None,  // no error
);

// Access completed subagent info
if let Some(manager) = internals.subagent_manager() {
    if let Some(completed) = manager.get_completed("researcher-1") {
        println!("Result: {:?}", completed.result);
        println!("Success: {}", completed.success);
    }

    // Get all completed subagents
    for info in manager.completed_subagents() {
        println!("{}: {:?}", info.session_id, info.result);
    }
}
```

### Direct SubAgentManager Access

```rust
// Get the SubAgentManager directly
if let Some(manager) = internals.subagent_manager() {
    // Check if a subagent exists
    if manager.exists("researcher-1") {
        println!("Subagent exists");
    }

    // Check if still active
    if manager.is_active("researcher-1") {
        println!("Still running");
    }

    // Get counts
    println!("Active: {}", manager.active_count());
    println!("Total: {}", manager.total_count());

    // Clean up completed subagents
    manager.clear_completed();
}
```

## Files Modified/Created

1. **`src/runtime/subagent_manager.rs`** (new)
   - `SubAgentManager` struct with active/completed tracking
   - `CompletedSubAgent` struct for completion info
   - Methods: `register`, `get`, `exists`, `is_active`, `mark_completed`, etc.

2. **`src/runtime/mod.rs`**
   - Added `pub mod subagent_manager`
   - Exported `SubAgentManager` and `CompletedSubAgent`

3. **`src/runtime/runtime.rs`**
   - Auto-creates `SubAgentManager` in agent context on spawn
   - Stores runtime reference in context for subagent spawning

4. **`src/runtime/internals.rs`**
   - Added `spawn_subagent()` helper method
   - Added `subagent_manager()`, `get_subagent()`, `active_subagents()`
   - Added `mark_subagent_completed()`

## Design Notes

1. **Automatic Registration**: When using `internals.spawn_subagent()`, the subagent handle is automatically registered with the parent's manager.

2. **Notification**: `OutputChunk::SubAgentSpawned` and `OutputChunk::SubAgentComplete` are sent when subagents are spawned/completed.

3. **Runtime in Context**: The `AgentRuntime` is stored in the context so agents can spawn subagents without needing external references.

4. **Thread Safety**: `SubAgentManager` uses `RwLock` for thread-safe access from multiple tasks.

5. **Completed Tracking**: Completed subagents are moved to a separate map, preserving their results while freeing the handle.
