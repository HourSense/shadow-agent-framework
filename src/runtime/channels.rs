//! Channel type definitions for agent communication
//!
//! Agents communicate via two channel types:
//! - **Input channel** (mpsc): Single-producer, single-consumer for sending commands to the agent
//! - **Output channel** (broadcast): Multi-consumer for streaming output to multiple subscribers

use tokio::sync::{broadcast, mpsc};

use crate::core::{InputMessage, OutputChunk};

/// Default buffer size for input channel
pub const INPUT_CHANNEL_SIZE: usize = 32;

/// Default buffer size for output broadcast channel
pub const OUTPUT_CHANNEL_SIZE: usize = 256;

// ============================================================================
// Channel Type Aliases
// ============================================================================

/// Sender half of the input channel (used by AgentHandle)
pub type InputSender = mpsc::Sender<InputMessage>;

/// Receiver half of the input channel (used by AgentInternals)
pub type InputReceiver = mpsc::Receiver<InputMessage>;

/// Sender half of the output broadcast channel (used by AgentInternals)
pub type OutputSender = broadcast::Sender<OutputChunk>;

/// Receiver half of the output broadcast channel (used by subscribers)
pub type OutputReceiver = broadcast::Receiver<OutputChunk>;

// ============================================================================
// Channel Creation
// ============================================================================

/// Create a new input channel pair
///
/// Returns (sender, receiver) for agent input.
/// The sender is used by `AgentHandle`, the receiver by `AgentInternals`.
pub fn create_input_channel() -> (InputSender, InputReceiver) {
    mpsc::channel(INPUT_CHANNEL_SIZE)
}

/// Create a new output broadcast channel
///
/// Returns the sender. Receivers are created by calling `sender.subscribe()`.
/// Multiple subscribers can receive the same output chunks.
pub fn create_output_channel() -> OutputSender {
    let (tx, _) = broadcast::channel(OUTPUT_CHANNEL_SIZE);
    tx
}

/// Create both input and output channels
///
/// Convenience function that returns all channel components needed for an agent.
pub fn create_agent_channels() -> (InputSender, InputReceiver, OutputSender) {
    let (input_tx, input_rx) = create_input_channel();
    let output_tx = create_output_channel();
    (input_tx, input_rx, output_tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_input_channel() {
        let (tx, mut rx) = create_input_channel();

        // Send input
        tx.send(InputMessage::UserInput("Hello".into()))
            .await
            .unwrap();

        // Receive input
        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, InputMessage::UserInput(s) if s == "Hello"));
    }

    #[tokio::test]
    async fn test_output_broadcast() {
        let tx = create_output_channel();

        // Create multiple subscribers
        let mut rx1 = tx.subscribe();
        let mut rx2 = tx.subscribe();

        // Send output
        tx.send(OutputChunk::TextDelta("Hi".into())).unwrap();

        // Both receivers get the message
        let chunk1 = rx1.recv().await.unwrap();
        let chunk2 = rx2.recv().await.unwrap();

        assert!(matches!(chunk1, OutputChunk::TextDelta(s) if s == "Hi"));
        assert!(matches!(chunk2, OutputChunk::TextDelta(s) if s == "Hi"));
    }

    #[tokio::test]
    async fn test_output_multiple_messages() {
        let tx = create_output_channel();
        let mut rx = tx.subscribe();

        // Send multiple messages
        tx.send(OutputChunk::TextDelta("One".into())).unwrap();
        tx.send(OutputChunk::TextDelta("Two".into())).unwrap();
        tx.send(OutputChunk::Done).unwrap();

        // Receive all
        let c1 = rx.recv().await.unwrap();
        let c2 = rx.recv().await.unwrap();
        let c3 = rx.recv().await.unwrap();

        assert!(matches!(c1, OutputChunk::TextDelta(s) if s == "One"));
        assert!(matches!(c2, OutputChunk::TextDelta(s) if s == "Two"));
        assert!(matches!(c3, OutputChunk::Done));
    }

    #[tokio::test]
    async fn test_create_agent_channels() {
        let (input_tx, mut input_rx, output_tx) = create_agent_channels();

        // Subscribe before sending
        let mut output_rx = output_tx.subscribe();

        // Test input
        input_tx
            .send(InputMessage::UserInput("Test".into()))
            .await
            .unwrap();
        let input = input_rx.recv().await.unwrap();
        assert!(matches!(input, InputMessage::UserInput(s) if s == "Test"));

        // Test output
        output_tx
            .send(OutputChunk::TextDelta("Response".into()))
            .unwrap();
        let output = output_rx.recv().await.unwrap();
        assert!(matches!(output, OutputChunk::TextDelta(s) if s == "Response"));
    }

    #[tokio::test]
    async fn test_input_channel_close() {
        let (tx, mut rx) = create_input_channel();

        // Drop sender
        drop(tx);

        // Receiver should get None
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_late_subscriber_misses_messages() {
        let tx = create_output_channel();

        // First subscriber gets all messages from this point
        let mut rx_early = tx.subscribe();

        // Send first message
        tx.send(OutputChunk::TextDelta("Early".into())).unwrap();

        // Late subscriber joins after first message
        let mut rx_late = tx.subscribe();

        // Send another message
        tx.send(OutputChunk::TextDelta("Late".into())).unwrap();

        // Early subscriber gets both messages
        let chunk1 = rx_early.recv().await.unwrap();
        let chunk2 = rx_early.recv().await.unwrap();
        assert!(matches!(chunk1, OutputChunk::TextDelta(s) if s == "Early"));
        assert!(matches!(chunk2, OutputChunk::TextDelta(s) if s == "Late"));

        // Late subscriber only gets the second message
        let chunk = rx_late.recv().await.unwrap();
        assert!(matches!(chunk, OutputChunk::TextDelta(s) if s == "Late"));
    }

    #[tokio::test]
    async fn test_send_without_subscribers() {
        let tx = create_output_channel();

        // Sending without subscribers returns error (0 receivers)
        let result = tx.send(OutputChunk::TextDelta("Nobody listening".into()));
        assert!(result.is_err());
    }
}
