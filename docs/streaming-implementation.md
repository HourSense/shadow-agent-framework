# Streaming Implementation

This document describes the streaming support added to the Shadow Agent SDK for the Anthropic API.

## Overview

The SDK now supports real-time streaming of responses from the Anthropic Messages API using Server-Sent Events (SSE). This allows applications to display response text as it's being generated, providing a better user experience for interactive applications.

## Key Components

### Types (`src/llm/types.rs`)

The following streaming types were added:

- **`StreamEvent`**: The main enum representing all possible SSE events:
  - `MessageStart`: Initial message metadata
  - `ContentBlockStart`: Start of a new content block (text, tool_use, thinking)
  - `ContentBlockDelta`: Incremental update to a content block
  - `ContentBlockStop`: End of a content block
  - `MessageDelta`: Final stop reason and cumulative usage
  - `MessageStop`: Stream complete
  - `Ping`: Keep-alive ping
  - `Error`: Error event

- **Delta Types** (`ContentDelta`):
  - `TextDelta`: Incremental text content
  - `InputJsonDelta`: Partial JSON for tool input
  - `ThinkingDelta`: Extended thinking content
  - `SignatureDelta`: Signature for thinking verification

- **`RawStreamEvent`**: Internal enum for deserializing SSE JSON data

### Methods (`src/llm/anthropic.rs`)

Two new streaming methods were added to `AnthropicProvider`:

#### `stream_message`

```rust
pub async fn stream_message(
    &self,
    user_message: &str,
    conversation_history: &[Message],
    system_prompt: Option<&str>,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>
```

Simple streaming without tools. Returns an async stream of `StreamEvent`.

#### `stream_with_tools`

```rust
pub async fn stream_with_tools(
    &self,
    messages: Vec<Message>,
    system_prompt: Option<&str>,
    tools: Vec<ToolDefinition>,
    tool_choice: Option<ToolChoice>,
    thinking: Option<ThinkingConfig>,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>
```

Streaming with full tool and extended thinking support.

## Event Flow

A typical streaming response follows this flow:

1. `MessageStart` - Contains message ID, model, and initial empty content
2. For each content block:
   - `ContentBlockStart` - Type of block (text, tool_use, thinking)
   - Multiple `ContentBlockDelta` events with incremental content
   - `ContentBlockStop` - End of the block
3. `MessageDelta` - Stop reason and cumulative token usage
4. `MessageStop` - Stream complete

Ping events may appear at any time for keep-alive.

## Usage Example

```rust
use futures::StreamExt;
use shadow_agent_sdk::llm::{AnthropicProvider, ContentDelta, StreamEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = AnthropicProvider::from_env()?;

    let mut stream = provider
        .stream_message("Hello, Claude!", &[], None)
        .await?;

    while let Some(event_result) = stream.next().await {
        match event_result? {
            StreamEvent::ContentBlockDelta(delta) => {
                if let ContentDelta::TextDelta { text } = delta.delta {
                    print!("{}", text);
                }
            }
            StreamEvent::MessageStop => {
                println!("\nStream complete!");
                break;
            }
            _ => {} // Handle other events as needed
        }
    }

    Ok(())
}
```

## Dependencies Added

The following dependencies were added to `Cargo.toml`:

- `reqwest` with `stream` feature - For HTTP streaming support
- `tokio-util` with `io` feature - For `StreamReader` to convert byte streams
- `async-stream` - For the `try_stream!` macro to create async streams

## Example

A complete example demonstrating streaming is available at:

```
examples/display_streaming/main.rs
```

Run it with:

```bash
cargo run --example display_streaming
```

Or with a custom message:

```bash
cargo run --example display_streaming -- "Explain quantum computing"
```

## Error Handling

The stream handles errors in several ways:

1. **API Errors**: If the initial request fails, an error is returned before streaming starts
2. **Stream Errors**: Errors during streaming are yielded as `StreamEvent::Error`
3. **Parse Errors**: If SSE parsing fails, a warning is logged but the stream continues
4. **Unknown Events**: Unknown event types are logged and skipped (forward compatibility)

## Agent-Level Streaming

The `StandardAgent` also supports streaming, which can be enabled via `AgentConfig`:

### Configuration

```rust
let config = AgentConfig::new("You are a helpful assistant")
    .with_tools(tools)
    .with_streaming(true); // Enable streaming

let agent = StandardAgent::new(config, llm);
```

### How Agent Streaming Works

When streaming is enabled on `StandardAgent`:

1. The agent uses `stream_with_tools` instead of `send_with_tools`
2. Text and thinking deltas are sent via `internals.send_text()` and `internals.send_thinking()` immediately as they arrive
3. Tool use blocks are accumulated and executed after the stream completes
4. The agent loop continues to work normally with tool calling

### Test Agent Example

The `test_agent` example supports streaming via the `--stream` flag and extended thinking via `--think`:

```bash
# Without streaming (default)
cargo run --example test_agent

# With streaming enabled
cargo run --example test_agent -- --stream

# With streaming and resume
cargo run --example test_agent -- --stream --resume

# With extended thinking enabled
cargo run --example test_agent -- --think

# With both streaming and thinking
cargo run --example test_agent -- --stream --think
```

### Output Chunks

When streaming, the agent sends `OutputChunk::TextDelta` and `OutputChunk::ThinkingDelta` chunks as they arrive. The `ConsoleRenderer` (or any other subscriber) receives these chunks and can display them immediately, providing real-time feedback to the user.

## Implementation Notes

- The streaming implementation uses SSE (Server-Sent Events) parsing with `tokio::io::BufReader`
- Events are parsed line-by-line, looking for `event:` and `data:` prefixes
- Empty lines signal the end of an event
- The `anthropic-beta` header is included for extended thinking support
- Agent streaming accumulates content blocks while streaming deltas to the output channel
- Tool execution happens after the stream completes (tools cannot be executed during streaming)
