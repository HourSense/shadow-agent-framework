//! AgentHandle - External interface for communicating with a running agent
//!
//! The `AgentHandle` is what external code (console, parent agent, tests) uses
//! to interact with a running agent. It provides methods to:
//! - Send input to the agent
//! - Subscribe to streaming output
//! - Check agent state
//! - Request interrupt or shutdown

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::{AgentState, FrameworkError, FrameworkResult, InputMessage};
use crate::tools::ToolResult;

use super::channels::{InputSender, OutputReceiver, OutputSender};

/// Handle for interacting with a running agent
///
/// This is the external interface for agent communication.
/// It can be cloned and shared across tasks.
#[derive(Clone)]
pub struct AgentHandle {
    /// Session ID of this agent
    session_id: String,

    /// Sender for input messages (to agent)
    input_tx: InputSender,

    /// Sender for output (for subscribing)
    output_tx: OutputSender,

    /// Current agent state
    state: Arc<RwLock<AgentState>>,
}

impl AgentHandle {
    /// Create a new agent handle
    ///
    /// This is typically called by `AgentRuntime::spawn()`, not directly.
    pub fn new(
        session_id: impl Into<String>,
        input_tx: InputSender,
        output_tx: OutputSender,
        state: Arc<RwLock<AgentState>>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            input_tx,
            output_tx,
            state,
        }
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    // =========================================================================
    // Input Methods
    // =========================================================================

    /// Send user input to the agent
    pub async fn send_input(&self, input: impl Into<String>) -> FrameworkResult<()> {
        self.send(InputMessage::UserInput(input.into())).await
    }

    /// Send a tool result to the agent
    ///
    /// Used when tools execute asynchronously and need to report back.
    pub async fn send_tool_result(
        &self,
        tool_use_id: impl Into<String>,
        result: ToolResult,
    ) -> FrameworkResult<()> {
        self.send(InputMessage::ToolResult {
            tool_use_id: tool_use_id.into(),
            result,
        })
        .await
    }

    /// Send a permission response to the agent
    pub async fn send_permission_response(
        &self,
        tool_name: impl Into<String>,
        allowed: bool,
        remember: bool,
    ) -> FrameworkResult<()> {
        self.send(InputMessage::PermissionResponse {
            tool_name: tool_name.into(),
            allowed,
            remember,
        })
        .await
    }

    /// Notify the agent that a subagent has completed
    pub async fn send_subagent_complete(
        &self,
        session_id: impl Into<String>,
        result: Option<String>,
    ) -> FrameworkResult<()> {
        self.send(InputMessage::SubAgentComplete {
            session_id: session_id.into(),
            result,
        })
        .await
    }

    /// Request graceful interrupt
    ///
    /// The agent should stop at the next safe point.
    pub async fn interrupt(&self) -> FrameworkResult<()> {
        self.send(InputMessage::Interrupt).await
    }

    /// Request shutdown
    ///
    /// The agent should terminate as soon as possible.
    pub async fn shutdown(&self) -> FrameworkResult<()> {
        self.send(InputMessage::Shutdown).await
    }

    /// Send any input message to the agent
    pub async fn send(&self, message: InputMessage) -> FrameworkResult<()> {
        self.input_tx
            .send(message)
            .await
            .map_err(|_| FrameworkError::ChannelClosed)
    }

    /// Try to send input without waiting (non-blocking)
    ///
    /// Returns an error if the channel is full or closed.
    pub fn try_send(&self, message: InputMessage) -> FrameworkResult<()> {
        self.input_tx
            .try_send(message)
            .map_err(|e| match e {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    FrameworkError::SendError("Channel full".into())
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => FrameworkError::ChannelClosed,
            })
    }

    // =========================================================================
    // Output Methods
    // =========================================================================

    /// Subscribe to agent output
    ///
    /// Returns a receiver that will get all output chunks from this point forward.
    /// Multiple subscribers can exist simultaneously.
    pub fn subscribe(&self) -> OutputReceiver {
        self.output_tx.subscribe()
    }

    /// Get the number of current subscribers
    pub fn subscriber_count(&self) -> usize {
        self.output_tx.receiver_count()
    }

    // =========================================================================
    // State Methods
    // =========================================================================

    /// Get the current agent state
    pub async fn state(&self) -> AgentState {
        self.state.read().await.clone()
    }

    /// Check if the agent is idle (waiting for input)
    pub async fn is_idle(&self) -> bool {
        matches!(*self.state.read().await, AgentState::Idle)
    }

    /// Check if the agent is processing
    pub async fn is_processing(&self) -> bool {
        matches!(*self.state.read().await, AgentState::Processing)
    }

    /// Check if the agent is done
    pub async fn is_done(&self) -> bool {
        matches!(*self.state.read().await, AgentState::Done)
    }

    /// Check if the agent has errored
    pub async fn is_error(&self) -> bool {
        matches!(*self.state.read().await, AgentState::Error { .. })
    }

    /// Check if the agent is still running (not done and not errored)
    pub async fn is_running(&self) -> bool {
        let state = self.state.read().await;
        !matches!(*state, AgentState::Done | AgentState::Error { .. })
    }

    /// Wait until the agent reaches a terminal state (Done or Error)
    ///
    /// This polls the state periodically. For event-driven waiting,
    /// subscribe to output and wait for `OutputChunk::Done` or `OutputChunk::Error`.
    pub async fn wait_for_completion(&self) {
        loop {
            if !self.is_running().await {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }
}

impl std::fmt::Debug for AgentHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandle")
            .field("session_id", &self.session_id)
            .field("subscriber_count", &self.output_tx.receiver_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::OutputChunk;
    use crate::runtime::channels::create_agent_channels;

    fn create_test_handle() -> (AgentHandle, super::super::channels::InputReceiver) {
        let (input_tx, input_rx, output_tx) = create_agent_channels();
        let state = Arc::new(RwLock::new(AgentState::Idle));
        let handle = AgentHandle::new("test-session", input_tx, output_tx, state);
        (handle, input_rx)
    }

    #[tokio::test]
    async fn test_send_input() {
        let (handle, mut rx) = create_test_handle();

        handle.send_input("Hello").await.unwrap();

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, InputMessage::UserInput(s) if s == "Hello"));
    }

    #[tokio::test]
    async fn test_interrupt() {
        let (handle, mut rx) = create_test_handle();

        handle.interrupt().await.unwrap();

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, InputMessage::Interrupt));
    }

    #[tokio::test]
    async fn test_shutdown() {
        let (handle, mut rx) = create_test_handle();

        handle.shutdown().await.unwrap();

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, InputMessage::Shutdown));
    }

    #[tokio::test]
    async fn test_subscribe() {
        let (handle, _rx) = create_test_handle();

        // Create subscribers
        let mut sub1 = handle.subscribe();
        let mut sub2 = handle.subscribe();

        assert_eq!(handle.subscriber_count(), 2);

        // Simulate agent sending output (normally done by AgentInternals)
        // We access the internal output_tx for testing
        handle
            .output_tx
            .send(OutputChunk::TextDelta("Hi".into()))
            .unwrap();

        // Both subscribers receive
        let chunk1 = sub1.recv().await.unwrap();
        let chunk2 = sub2.recv().await.unwrap();

        assert!(matches!(chunk1, OutputChunk::TextDelta(s) if s == "Hi"));
        assert!(matches!(chunk2, OutputChunk::TextDelta(s) if s == "Hi"));
    }

    #[tokio::test]
    async fn test_state() {
        let (input_tx, _input_rx, output_tx) = create_agent_channels();
        let state = Arc::new(RwLock::new(AgentState::Idle));
        let handle = AgentHandle::new("test", input_tx, output_tx, state.clone());

        assert!(handle.is_idle().await);
        assert!(handle.is_running().await);

        // Change state
        *state.write().await = AgentState::Processing;
        assert!(handle.is_processing().await);

        *state.write().await = AgentState::Done;
        assert!(handle.is_done().await);
        assert!(!handle.is_running().await);
    }

    #[tokio::test]
    async fn test_session_id() {
        let (handle, _rx) = create_test_handle();
        assert_eq!(handle.session_id(), "test-session");
    }

    #[tokio::test]
    async fn test_send_permission_response() {
        let (handle, mut rx) = create_test_handle();

        handle
            .send_permission_response("Bash", true, false)
            .await
            .unwrap();

        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            InputMessage::PermissionResponse {
                tool_name,
                allowed: true,
                remember: false,
            } if tool_name == "Bash"
        ));
    }

    #[tokio::test]
    async fn test_clone() {
        let (handle1, mut rx) = create_test_handle();
        let handle2 = handle1.clone();

        // Both handles point to same session
        assert_eq!(handle1.session_id(), handle2.session_id());

        // Sending from either works
        handle1.send_input("From 1").await.unwrap();
        handle2.send_input("From 2").await.unwrap();

        let msg1 = rx.recv().await.unwrap();
        let msg2 = rx.recv().await.unwrap();

        assert!(matches!(msg1, InputMessage::UserInput(s) if s == "From 1"));
        assert!(matches!(msg2, InputMessage::UserInput(s) if s == "From 2"));
    }
}
