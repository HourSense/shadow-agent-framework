//! Session management for agents
//!
//! This module provides `AgentSession` for managing agent conversations,
//! history, metadata, and persistence.
//!
//! Each agent has its own session with a unique session_id. Sessions can
//! be linked via parent/child relationships for subagent tracking.

pub mod metadata;
pub mod session;
pub mod storage;

pub use metadata::SessionMetadata;
pub use session::AgentSession;
pub use storage::SessionStorage;
