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
//! ## Recommended: Custom Transport (Full Control)
//!
//! For maximum flexibility (auth headers, custom settings, etc.):
//!
//! ```ignore
//! use shadow_agent_sdk::mcp::{MCPServerManager, MCPToolProvider};
//! use shadow_agent_sdk::tools::ToolRegistry;
//! use rmcp::transport::StreamableHttpClientTransport;
//! use rmcp::ServiceExt;
//! use std::sync::Arc;
//!
//! // Create transport with custom configuration
//! let transport = StreamableHttpClientTransport::from_uri("http://localhost:8005/mcp")
//!     .with_header("Authorization", "Bearer your-token")
//!     .with_header("X-Custom", "value");
//!
//! let service = ().serve(transport).await?;
//!
//! // Add to manager
//! let mcp_manager = Arc::new(MCPServerManager::new());
//! mcp_manager.add_service("filesystem", service).await?;
//!
//! // Add to tool registry
//! let mcp_provider = Arc::new(MCPToolProvider::new(mcp_manager));
//! tool_registry.add_provider(mcp_provider).await?;
//! ```
//!
//! ## Simple: URI-based Configuration
//!
//! For simple cases without auth:
//!
//! ```ignore
//! use shadow_agent_sdk::mcp::{MCPServerManager, MCPServerConfig, MCPToolProvider};
//!
//! let mcp_manager = Arc::new(MCPServerManager::new());
//! mcp_manager.add_server(MCPServerConfig::new(
//!     "filesystem",
//!     "http://localhost:8005/mcp"
//! )).await?;
//!
//! let mcp_provider = Arc::new(MCPToolProvider::new(mcp_manager));
//! tool_registry.add_provider(mcp_provider).await?;
//! ```
//!
//! # Tool Namespacing
//!
//! MCP tools are automatically namespaced with their server ID to avoid conflicts:
//! - Server ID: `filesystem`
//! - Original tool name: `read_file`
//! - Exposed name: `filesystem__read_file`

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
