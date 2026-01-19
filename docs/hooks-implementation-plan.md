# Hooks Implementation Plan (v3 - Simplified)

## Design Philosophy

**Keep it simple:**
- Pass in a mutable context - hook can read/modify anything it needs
- Return only what's necessary (permission decision)
- In-place modification - if hook changes messages, they're changed

## Core Types

### HookEvent

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
}
```

### HookContext - Mutable access to everything

```rust
/// Mutable context passed to hooks
/// Hook can read/modify anything here
pub struct HookContext<'a> {
    /// The hook event type
    pub event: HookEvent,

    /// Full agent internals - messages, session, context, etc.
    pub internals: &'a mut AgentInternals,

    // === Tool-specific (populated for tool hooks) ===
    /// Tool name (for tool hooks)
    pub tool_name: Option<String>,
    /// Tool input - MUTABLE, hook can modify this
    pub tool_input: Option<serde_json::Value>,
    /// Tool use ID
    pub tool_use_id: Option<String>,

    // === Results (for post hooks) ===
    /// Tool result (PostToolUse)
    pub tool_result: Option<ToolResult>,
    /// Error (PostToolUseFailure)
    pub error: Option<String>,

    // === User input (for UserPromptSubmit) ===
    /// User prompt - MUTABLE, hook can modify this
    pub user_prompt: Option<String>,
}

impl<'a> HookContext<'a> {
    /// Get messages (shorthand)
    pub fn messages(&self) -> &[Message] {
        self.internals.session.history()
    }

    /// Get messages mutably - modify in place
    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        self.internals.session.history_mut()
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        self.internals.session_id()
    }

    /// Get/set metadata
    pub fn metadata(&self) -> &HashMap<String, Value> { ... }
    pub fn set_metadata(&mut self, key: &str, value: Value) { ... }
}
```

### HookResult - Simple output

```rust
/// Result from a hook - just permission + optional reason
#[derive(Debug, Clone, Default)]
pub struct HookResult {
    /// Permission decision (for PreToolUse)
    pub decision: Option<PermissionDecision>,
    /// Reason for decision
    pub reason: Option<String>,
}

impl HookResult {
    /// Allow the operation
    pub fn allow() -> Self {
        Self { decision: Some(PermissionDecision::Allow), reason: None }
    }

    /// Deny the operation
    pub fn deny(reason: impl Into<String>) -> Self {
        Self { decision: Some(PermissionDecision::Deny), reason: Some(reason.into()) }
    }

    /// Use default permission flow
    pub fn ask() -> Self {
        Self { decision: Some(PermissionDecision::Ask), reason: None }
    }

    /// No decision - continue normally
    pub fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PermissionDecision {
    Allow,  // Skip permission check, execute
    Deny,   // Block
    Ask,    // Normal permission flow
}
```

### Hook Callback

```rust
/// Simple async hook function
pub type HookFn = Box<dyn Fn(&mut HookContext) -> BoxFuture<'_, HookResult> + Send + Sync>;

/// Or as a trait for more flexibility
pub trait Hook: Send + Sync {
    fn call<'a>(&self, ctx: &'a mut HookContext<'a>) -> BoxFuture<'a, HookResult>;
}
```

### HookMatcher

```rust
pub struct HookMatcher {
    /// Regex pattern for tool names (None = match all)
    pattern: Option<Regex>,
    /// The hook function
    hook: Arc<dyn Hook>,
}

impl HookMatcher {
    pub fn new<H: Hook + 'static>(hook: H) -> Self { ... }
    pub fn with_pattern<H: Hook + 'static>(pattern: &str, hook: H) -> Result<Self> { ... }
    pub fn matches(&self, tool_name: &str) -> bool { ... }
}
```

### HookRegistry

```rust
pub struct HookRegistry {
    hooks: HashMap<HookEvent, Vec<HookMatcher>>,
}

impl HookRegistry {
    pub fn new() -> Self { ... }

    pub fn add<H: Hook + 'static>(&mut self, event: HookEvent, hook: H) -> &mut Self { ... }

    pub fn add_with_pattern<H: Hook + 'static>(
        &mut self,
        event: HookEvent,
        pattern: &str,
        hook: H,
    ) -> Result<&mut Self> { ... }

    /// Run hooks, return combined result (Deny wins)
    pub async fn run(&self, ctx: &mut HookContext<'_>) -> HookResult { ... }
}
```

## File Structure

```
src/hooks/
├── mod.rs       # Exports
├── types.rs     # HookEvent, HookContext, HookResult, PermissionDecision
├── registry.rs  # HookRegistry, HookMatcher
```

## Integration

### AgentConfig

```rust
pub struct AgentConfig {
    // ... existing ...
    pub hooks: Option<Arc<HookRegistry>>,
}

impl AgentConfig {
    pub fn with_hooks(mut self, hooks: HookRegistry) -> Self {
        self.hooks = Some(Arc::new(hooks));
        self
    }
}
```

### ToolExecutor

```rust
// PreToolUse
let mut ctx = HookContext::pre_tool_use(internals, tool_name, &input, tool_id);
let result = hooks.run(&mut ctx).await;

// Hook may have modified ctx.tool_input - use it
let final_input = ctx.tool_input.unwrap_or(input);

match result.decision {
    Some(PermissionDecision::Deny) => return ToolResult::error(result.reason.unwrap_or("Blocked")),
    Some(PermissionDecision::Allow) => { /* skip permission check, execute */ },
    _ => { /* normal permission flow */ }
}
```

### StandardAgent

```rust
// UserPromptSubmit
let mut ctx = HookContext::user_prompt_submit(internals, &text);
let result = hooks.run(&mut ctx).await;

// Hook may have modified ctx.user_prompt
let final_prompt = ctx.user_prompt.unwrap_or(text);
```

## Usage Examples

### Block Dangerous Commands

```rust
hooks.add_with_pattern(HookEvent::PreToolUse, "Bash", |ctx| async move {
    let cmd = ctx.tool_input.as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if cmd.contains("rm -rf") {
        HookResult::deny("Dangerous command")
    } else {
        HookResult::none()
    }
})?;
```

### Rewrite Paths

```rust
hooks.add_with_pattern(HookEvent::PreToolUse, "Read|Write|Edit", |ctx| async move {
    if let Some(ref mut input) = ctx.tool_input {
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            input["file_path"] = json!(format!("/mnt/remote{}", path));
        }
    }
    HookResult::allow()  // Modified input, allow
})?;
```

### Filter Messages

```rust
hooks.add(HookEvent::PreToolUse, |ctx| async move {
    // Modify messages in place
    ctx.messages_mut().retain(|m| !contains_secret(m));
    HookResult::none()
});
```

### Auto-Approve Read Tools

```rust
hooks.add_with_pattern(HookEvent::PreToolUse, "Read|Glob|Grep", |_ctx| async move {
    HookResult::allow()
})?;
```

### Audit Logging

```rust
hooks.add(HookEvent::PostToolUse, |ctx| async move {
    tracing::info!(
        tool = ?ctx.tool_name,
        result = ?ctx.tool_result,
        "Tool executed"
    );
    HookResult::none()
});
```

## Implementation Order

1. `src/hooks/types.rs` - HookEvent, HookContext, HookResult, PermissionDecision
2. `src/hooks/registry.rs` - HookRegistry, HookMatcher, Hook trait
3. `src/hooks/mod.rs` - Exports
4. `src/agent/config.rs` - Add hooks field
5. `src/agent/executor.rs` - Integrate tool hooks
6. `src/agent/standard_loop.rs` - Integrate UserPromptSubmit
7. `src/lib.rs` - Export hooks
8. Example in test_agent

## Summary

| What | How |
|------|-----|
| Hook gets | `&mut HookContext` - can read/modify everything |
| Hook returns | `HookResult` - just permission decision |
| Modify tool input | `ctx.tool_input = Some(new_value)` |
| Modify messages | `ctx.messages_mut().push(...)` or `.retain(...)` |
| Block operation | `HookResult::deny("reason")` |
| Allow operation | `HookResult::allow()` |
| Normal flow | `HookResult::none()` |
