//! MCP (Model Context Protocol) Support
//!
//! This module provides integration with MCP servers, allowing agents to use
//! tools from external MCP servers as if they were native tools.
//!
//! # Architecture
//!
//! - `MCPServer`: Wraps rmcp service, manages connection to a single MCP server
//! - `MCPServerManager`: Manages multiple MCP servers
//! - `MCPToolAdapter`: Adapts MCP tools to implement the Tool trait
//! - `MCPToolProvider`: Implements ToolProvider to expose MCP tools to the registry
//!
//! # Usage
//!
//! ```ignore
//! use shadow_agent_sdk::mcp::{MCPConfig, MCPServerConfig};
//! use shadow_agent_sdk::agent::AgentConfig;
//!
//! // Configure MCP servers
//! let mcp_config = MCPConfig::new()
//!     .add_server(MCPServerConfig::new(
//!         "filesystem",
//!         "http://localhost:8005/mcp"
//!     ));
//!
//! // Create agent with MCP support
//! let config = AgentConfig::new("You are helpful")
//!     .with_mcp_servers(mcp_config)
//!     .await?;
//! ```
//!
//! # Tool Namespacing
//!
//! MCP tools are automatically namespaced with their server ID to avoid conflicts:
//! - Server ID: `filesystem`
//! - Original tool name: `read_file`
//! - Exposed name: `filesystem:read_file`

mod config;
mod manager;
mod provider;
mod server;
mod tool_adapter;

// Public exports
pub use config::{MCPConfig, MCPServerConfig};
pub use manager::{MCPServerManager, MCPToolInfo};
pub use provider::MCPToolProvider;
pub use server::MCPServer;
pub use tool_adapter::MCPToolAdapter;
