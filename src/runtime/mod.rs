//! Agent runtime and communication
//!
//! This module provides the infrastructure for running agents:
//! - `AgentRuntime` - Spawns and manages agent tasks
//! - `AgentHandle` - External interface for communicating with a running agent
//! - `AgentInternals` - Internal state passed to agent functions
//! - Channel types for input/output communication
//!
//! Agents run as separate tokio tasks and communicate via channels.
//! The `AgentHandle` allows sending input and subscribing to streaming output.

pub mod channels;
pub mod handle;
pub mod internals;
pub mod runtime;

pub use channels::{InputReceiver, InputSender, OutputReceiver, OutputSender};
pub use handle::AgentHandle;
pub use internals::AgentInternals;
pub use runtime::AgentRuntime;
