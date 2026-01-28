//! MCP Server wrapper
//!
//! Wraps rmcp service to manage connections to individual MCP servers

use anyhow::{anyhow, Result};
use rmcp::model::{CallToolRequestParams, CallToolResult, ListToolsResult, Tool};
use rmcp::service::RunningService;
use rmcp::transport::{
    streamable_http_client::StreamableHttpClientTransportConfig,
    StreamableHttpClientTransport,
};
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Map, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::config::MCPServerConfig;

/// The concrete transport type we use for HTTP MCP connections
pub type HttpClientTransport = StreamableHttpClientTransport<reqwest::Client>;

/// Wrapper around an rmcp service connection
#[derive(Debug)]
pub struct MCPServer {
    /// Unique identifier for this server
    id: String,

    /// URI of the server
    uri: String,

    /// The underlying rmcp service (None if not connected)
    service: Arc<RwLock<Option<RunningService<RoleClient, ()>>>>,

    /// Configuration
    config: MCPServerConfig,
}

impl MCPServer {
    /// Create a new MCP server and connect to it
    pub async fn new(config: MCPServerConfig) -> Result<Self> {
        let id = config.id.clone();
        let uri = config.uri.clone();

        tracing::info!("[MCPServer] Connecting to '{}' at {}", id, uri);

        let service = Self::create_service(&uri).await?;

        Ok(Self {
            id,
            uri,
            service: Arc::new(RwLock::new(Some(service))),
            config,
        })
    }

    /// Create an rmcp service connection
    async fn create_service(uri: &str) -> Result<RunningService<RoleClient, ()>> {
        let transport_config = StreamableHttpClientTransportConfig::with_uri(uri);
        let transport: HttpClientTransport = HttpClientTransport::from_config(transport_config);

        // Serve the transport to create a service
        let service = ().serve(transport).await?;

        Ok(service)
    }

    /// Get the server ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the server URI
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Check if the server is connected
    pub async fn is_connected(&self) -> bool {
        self.service.read().await.is_some()
    }

    /// Reconnect to the server
    pub async fn reconnect(&self) -> Result<()> {
        tracing::info!("[MCPServer] Reconnecting to '{}'", self.id);

        let mut service_guard = self.service.write().await;

        // Drop old connection
        *service_guard = None;

        // Create new connection
        let service = Self::create_service(&self.uri).await?;
        *service_guard = Some(service);

        tracing::info!("[MCPServer] Successfully reconnected to '{}'", self.id);
        Ok(())
    }

    /// List all tools available on this server
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let service_guard = self.service.read().await;
        let service = service_guard
            .as_ref()
            .ok_or_else(|| anyhow!("MCP server '{}' is not connected", self.id))?;

        tracing::debug!("[MCPServer] Listing tools from '{}'", self.id);

        let result: ListToolsResult = service.list_tools(Default::default()).await?;

        tracing::info!(
            "[MCPServer] Got {} tools from '{}'",
            result.tools.len(),
            self.id
        );

        Ok(result.tools)
    }

    /// Call a tool on this server
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Map<String, Value>>,
    ) -> Result<CallToolResult> {
        let service_guard = self.service.read().await;
        let service = service_guard
            .as_ref()
            .ok_or_else(|| anyhow!("MCP server '{}' is not connected", self.id))?;

        tracing::info!(
            "[MCPServer] Calling tool '{}' on server '{}'",
            name,
            self.id
        );
        tracing::debug!("[MCPServer] Arguments: {:?}", arguments);

        let result = service
            .call_tool(CallToolRequestParams {
                meta: None,
                name: name.to_string().into(),
                arguments,
                task: None,
            })
            .await?;

        tracing::debug!("[MCPServer] Tool call completed for '{}'", name);

        Ok(result)
    }

    /// Health check - try to list tools to verify connection
    pub async fn health_check(&self) -> Result<()> {
        self.list_tools().await?;
        Ok(())
    }

    /// Get configuration
    pub fn config(&self) -> &MCPServerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires a running MCP server
    async fn test_mcp_server_connection() {
        let config = MCPServerConfig::new("test-server", "http://localhost:8005/mcp");

        let server = MCPServer::new(config).await.unwrap();
        assert!(server.is_connected().await);

        let tools = server.list_tools().await.unwrap();
        assert!(!tools.is_empty());
    }
}
