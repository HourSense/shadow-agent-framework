//! Core types for the agent framework
//!
//! This module provides the fundamental types used throughout the framework:
//! - `AgentContext` - Hidden state passed to tools
//! - `AgentState` - Current state of an agent
//! - `OutputChunk` / `InputMessage` - Communication types
//! - `FrameworkError` - Error types

pub mod context;
pub mod error;
pub mod output;
pub mod state;

pub use context::{AgentContext, ResourceMap};
pub use error::{FrameworkError, FrameworkResult};
pub use output::{InputMessage, OutputChunk};
pub use state::AgentState;
