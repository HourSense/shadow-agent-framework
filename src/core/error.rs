//! Framework error types

use thiserror::Error;

/// Errors that can occur in the agent framework
#[derive(Error, Debug)]
pub enum FrameworkError {
    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Agent is not running
    #[error("Agent not running: {0}")]
    AgentNotRunning(String),

    /// Agent is already running
    #[error("Agent already running: {0}")]
    AgentAlreadyRunning(String),

    /// Channel closed unexpectedly
    #[error("Channel closed")]
    ChannelClosed,

    /// Send error on channel
    #[error("Failed to send message: {0}")]
    SendError(String),

    /// Receive error on channel
    #[error("Failed to receive message: {0}")]
    ReceiveError(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Tool execution error
    #[error("Tool error: {0}")]
    ToolError(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Agent was interrupted
    #[error("Agent interrupted")]
    Interrupted,

    /// Agent shutdown requested
    #[error("Agent shutdown")]
    Shutdown,

    /// Generic error with message
    #[error("{0}")]
    Other(String),
}

impl FrameworkError {
    /// Create a generic error from a string
    pub fn other(msg: impl Into<String>) -> Self {
        FrameworkError::Other(msg.into())
    }

    /// Create a tool error
    pub fn tool_error(msg: impl Into<String>) -> Self {
        FrameworkError::ToolError(msg.into())
    }
}

/// Result type alias for framework operations
pub type FrameworkResult<T> = Result<T, FrameworkError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = FrameworkError::SessionNotFound("abc123".into());
        assert_eq!(err.to_string(), "Session not found: abc123");

        let err = FrameworkError::ChannelClosed;
        assert_eq!(err.to_string(), "Channel closed");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let framework_err: FrameworkError = io_err.into();
        assert!(matches!(framework_err, FrameworkError::Io(_)));
    }
}
