# Extended Thinking Implementation

This document describes the extended thinking support added to the Shadow Agent SDK.

## Overview

Extended thinking gives Claude enhanced reasoning capabilities for complex tasks, showing its step-by-step thought process before delivering its final answer. The SDK now supports extended thinking at both the LLM provider level and the agent level.

## How Extended Thinking Works

When extended thinking is enabled, Claude creates `thinking` content blocks containing its internal reasoning. The API response includes:

1. `thinking` blocks - Claude's step-by-step reasoning
2. `text` blocks - The final response

## Configuration

### ThinkingConfig Type

The SDK defines a `ThinkingConfig` type in `src/llm/types.rs`:

```rust
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,  // Always "enabled"
    pub budget_tokens: u32,     // Token budget for thinking (minimum 1024)
}
```

### AgentConfig Builder Methods

The `AgentConfig` provides two builder methods for enabling thinking:

```rust
// Simple method - just specify token budget
let config = AgentConfig::new("You are helpful")
    .with_thinking(16000);  // 16k token budget

// Advanced method - provide full ThinkingConfig
let config = AgentConfig::new("You are helpful")
    .with_thinking_config(ThinkingConfig::enabled(10000));
```

## Usage Examples

### Agent-Level Thinking

Enable thinking on the `StandardAgent` via `AgentConfig`:

```rust
use shadow_agent_sdk::agent::{AgentConfig, StandardAgent};

let config = AgentConfig::new("You are a helpful assistant")
    .with_tools(tools)
    .with_thinking(16000)     // Enable thinking with 16k token budget
    .with_streaming(true);    // Streaming works with thinking

let agent = StandardAgent::new(config, llm);
```

### Test Agent Example

The `test_agent` example supports thinking via the `--think` flag:

```bash
# Enable extended thinking
cargo run --example test_agent -- --think

# With streaming and thinking
cargo run --example test_agent -- --stream --think

# With streaming, thinking, and session resume
cargo run --example test_agent -- --stream --think --resume
```

### LLM Provider Level

You can also use thinking directly with the LLM provider:

```rust
use shadow_agent_sdk::llm::{AnthropicProvider, ThinkingConfig};

let provider = AnthropicProvider::from_env()?;

// Non-streaming with thinking
let response = provider
    .send_with_tools(
        messages,
        Some("System prompt"),
        tools,
        None,
        Some(ThinkingConfig::enabled(16000)),
    )
    .await?;

// Streaming with thinking
let stream = provider
    .stream_with_tools(
        messages,
        Some("System prompt"),
        tools,
        None,
        Some(ThinkingConfig::enabled(16000)),
    )
    .await?;
```

## Streaming with Thinking

When both streaming and thinking are enabled:

1. Thinking content is streamed via `ThinkingDelta` events
2. The agent calls `internals.send_thinking(text)` for each thinking delta
3. Text content follows via `TextDelta` events
4. The `ConsoleRenderer` displays thinking content (if `show_thinking(true)` is set)

### Handling Thinking in Stream Events

```rust
while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::ContentBlockDelta(delta) => {
            match delta.delta {
                ContentDelta::ThinkingDelta { thinking } => {
                    // Display thinking content
                    print!("[Thinking] {}", thinking);
                }
                ContentDelta::TextDelta { text } => {
                    // Display response text
                    print!("{}", text);
                }
                _ => {}
            }
        }
        _ => {}
    }
}
```

## Conversation History

Thinking blocks are stored in conversation history just like text blocks:

```rust
// Assistant message may contain thinking and text blocks
ContentBlock::Thinking { thinking, signature } => {
    // Thinking content with signature for verification
}
ContentBlock::Text { text } => {
    // Regular text response
}
```

When passing messages back to the API (e.g., for tool use), thinking blocks must be preserved to maintain reasoning continuity.

## Best Practices

1. **Budget Sizing**: Start with 16k tokens for complex tasks. The minimum is 1024 tokens.

2. **Use with Complex Tasks**: Extended thinking is most valuable for:
   - Mathematical reasoning
   - Code analysis and debugging
   - Multi-step problem solving
   - Complex analysis tasks

3. **Streaming Recommended**: For better user experience, enable streaming when using thinking so users can see the reasoning process in real-time.

4. **Token Considerations**:
   - Thinking tokens count against your output token limit
   - Claude may not use the entire budget for simpler tasks
   - Monitor token usage for cost optimization

5. **Tool Use**: When using tools with thinking, thinking blocks from the last assistant turn must be preserved when sending tool results.

## Supported Models

Extended thinking is supported in Claude 4+ models:
- Claude Sonnet 4.5
- Claude Sonnet 4
- Claude Haiku 4.5
- Claude Opus 4.5
- Claude Opus 4.1
- Claude Opus 4

## Implementation Details

### StandardAgent Changes

The `StandardAgent` in `src/agent/standard_loop.rs` passes the thinking config to both streaming and non-streaming LLM calls:

```rust
// Non-streaming
let response = self.llm.send_with_tools(
    messages,
    Some(&self.config.system_prompt),
    tool_definitions.to_vec(),
    None,
    self.config.thinking.clone(),  // Pass thinking config
).await?;

// Streaming
let stream = self.llm.stream_with_tools(
    messages,
    Some(&self.config.system_prompt),
    tool_definitions.to_vec(),
    None,
    self.config.thinking.clone(),  // Pass thinking config
).await?;
```

### Output Chunks

The agent sends thinking content to subscribers via `OutputChunk::ThinkingDelta`:

```rust
// In streaming mode
ContentDelta::ThinkingDelta { thinking } => {
    thinking_accum.push_str(thinking);
    internals.send_thinking(thinking);
}

// In non-streaming mode
ContentBlock::Thinking { thinking, .. } => {
    internals.send_thinking(thinking);
}
```

## CLI Display

The CLI renderer (`ConsoleRenderer`) displays thinking content in real-time when enabled:

```rust
let renderer = ConsoleRenderer::new(handle)
    .show_thinking(true);  // Enable thinking display
```

The thinking output flow works as follows:

1. When the first `ThinkingDelta` arrives, CLI prints a thinking header
2. Each `ThinkingDelta` chunk is printed immediately as it streams in
3. When `ThinkingComplete` is received, CLI prints a footer to close the thinking block
4. The response text then streams after the thinking block

This provides real-time visibility into Claude's reasoning process as it happens, similar to how text responses are streamed.

The Console provides these methods for thinking display:
- `print_thinking_prefix()` - Prints the header with "💭 Agent Thinking:"
- `print_thinking_chunk()` - Prints a chunk of thinking content (italic, dim)
- `print_thinking_suffix()` - Prints the closing footer

## See Also

- [Streaming Implementation](./streaming-implementation.md) - Details on streaming support
- [Anthropic Extended Thinking Docs](./anthropic-extended-thinking.md) - Official Anthropic documentation
