# Hooks Implementation

## Overview

Implemented a hooks system that allows intercepting and controlling agent behavior at key execution points. The implementation follows the simplified design from `hooks-implementation-plan.md`.

## Files Created/Modified

### New Files

1. **`src/hooks/types.rs`**
   - `HookEvent` enum: PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit
   - `HookContext<'a>` struct with mutable access to agent internals
   - `HookResult` with permission decision (Allow/Deny/Ask)
   - `PermissionDecision` enum

2. **`src/hooks/registry.rs`**
   - `Hook` trait for implementing hooks
   - `HookMatcher` for regex-based tool name matching
   - `HookRegistry` for storing and running hooks
   - Uses Higher-Ranked Trait Bounds (HRTB) for closure support

3. **`src/hooks/mod.rs`**
   - Module exports with comprehensive documentation

### Modified Files

1. **`src/lib.rs`**
   - Added `pub mod hooks;` export

2. **`src/agent/config.rs`**
   - Added `hooks: Option<Arc<HookRegistry>>` field
   - Added `with_hooks(hooks: HookRegistry)` builder method

3. **`src/agent/executor.rs`**
   - Integrated PreToolUse hooks (can block, allow, or modify input)
   - Integrated PostToolUse hooks (for logging/observation)
   - Integrated PostToolUseFailure hooks (for error logging)

4. **`src/agent/standard_loop.rs`**
   - Integrated UserPromptSubmit hooks (can modify or block user prompts)

5. **`examples/test_agent/main.rs`**
   - Added example hooks:
     - Block dangerous Bash commands (`rm -rf /`, fork bombs)
     - Auto-approve read-only tools (Read, Glob, Grep)

## Design Decisions

### Synchronous Hooks
Made hooks synchronous instead of async to avoid lifetime complexity with mutable borrows. If async operations are needed (like HTTP calls), users can spawn a task.

### HRTB for Closures
Used Higher-Ranked Trait Bounds (`for<'a> Fn(&mut HookContext<'a>)`) to allow closures to work with any lifetime of HookContext. This requires explicit type annotations in some cases.

### Result Combination
When multiple hooks run, results are combined with priority: Deny > Allow > Ask > None.

## Usage

```rust
use shadow_agent_sdk::hooks::{HookContext, HookEvent, HookRegistry, HookResult};

let mut hooks = HookRegistry::new();

// Block dangerous commands
hooks.add_with_pattern(HookEvent::PreToolUse, "Bash", |ctx: &mut HookContext| {
    let cmd = ctx.tool_input.as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if cmd.contains("rm -rf /") {
        HookResult::deny("Dangerous command blocked")
    } else {
        HookResult::none()
    }
}).expect("Invalid regex");

// Auto-approve read-only tools
hooks.add_with_pattern(HookEvent::PreToolUse, "^(Read|Glob|Grep)$", |_ctx: &mut HookContext| {
    HookResult::allow()
}).expect("Invalid regex");

// Add to agent config
let config = AgentConfig::new("System prompt")
    .with_tools(tools)
    .with_hooks(hooks);
```

## Hook Events

| Event | When | Can Modify |
|-------|------|------------|
| `PreToolUse` | Before tool executes | `tool_input`, can block/allow |
| `PostToolUse` | After tool succeeds | Observation only |
| `PostToolUseFailure` | After tool fails | Observation only |
| `UserPromptSubmit` | When user sends prompt | `user_prompt`, can block |

## HookResult Options

| Method | Effect |
|--------|--------|
| `HookResult::none()` | Continue normally |
| `HookResult::allow()` | Skip permission check, execute tool |
| `HookResult::deny("reason")` | Block tool, return error to LLM |
| `HookResult::ask()` | Use normal permission flow |

## Important Notes

1. Closures need explicit type annotations: `|ctx: &mut HookContext| { ... }`
2. Hooks are synchronous - spawn tasks for async work
3. PreToolUse hooks can modify `ctx.tool_input` in place
4. UserPromptSubmit hooks can modify `ctx.user_prompt` in place
