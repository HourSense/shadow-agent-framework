# User Interrupt Implementation

## Overview

Added support for users to interrupt the agent during LLM streaming. When interrupted, the agent:
1. Stops the current stream
2. Saves any partial content
3. Adds a message indicating the interruption
4. Returns control to the user

## How It Works

### Sending an Interrupt

From the frontend/CLI, call:
```rust
handle.interrupt().await?;
```

This sends `InputMessage::Interrupt` to the agent.

### During Streaming

The streaming loop uses `tokio::select!` to check for interrupts while processing stream events:

```rust
loop {
    tokio::select! {
        biased;  // Check interrupt first

        // Check for interrupt
        maybe_input = async { internals.try_receive() } => {
            if let Some(InputMessage::Interrupt) = maybe_input {
                was_interrupted = true;
                // Save partial content
                break;
            }
        }

        // Process stream event
        event_result = stream.next() => {
            // ... handle stream event
        }
    }
}
```

### When Interrupted

1. **Partial content is saved**: Any text/thinking accumulated so far is added to content_blocks
2. **Session is updated**:
   - Partial assistant message is added (if any content)
   - A user message `[User interrupted the response]` is added
3. **Turn ends**: The agent returns to idle state

## Files Modified

1. **`src/agent/standard_loop.rs`**
   - Updated `call_llm_streaming()` to return `(blocks, stop_reason, was_interrupted)`
   - Added `tokio::select!` to check for interrupts during streaming
   - Added interrupt handling after LLM call

## Usage Example

### Programmatic (Tauri/Frontend)

```rust
// In your frontend/Tauri app
let handle = agent_handle.clone();

// User clicks "Stop" button
tokio::spawn(async move {
    handle.interrupt().await.ok();
});
```

## Limitations

- **Streaming only**: Interrupt checking only works during streaming mode (`with_streaming(true)`)
- **Not during tool execution**: If the agent is executing a tool, the interrupt will be queued until the tool completes
- **Graceful**: The agent finishes cleanly; partial content is preserved

## Note on Escape Key Support

Escape key interrupt support in the CLI was attempted but removed due to terminal compatibility issues with raw mode. The programmatic interrupt via `handle.interrupt().await` remains the recommended approach for interrupting agents.

## Future Improvements

- Add interrupt support during non-streaming mode
- Add interrupt support during tool execution (with tool cancellation)
- Add interrupt support during permission prompts
