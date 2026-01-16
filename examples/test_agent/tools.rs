//! Tool setup for the test agent
//!
//! Registers Read, Write, and Bash tools.

use anyhow::Result;
use singapore_project::tools::{BashTool, ReadTool, ToolRegistry, WriteTool};

/// Create a tool registry with Read, Write, and Bash tools
pub fn create_registry() -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();

    // Register tools
    registry.register(ReadTool::new()?);
    registry.register(WriteTool::new()?);
    registry.register(BashTool::new()?);

    Ok(registry)
}

