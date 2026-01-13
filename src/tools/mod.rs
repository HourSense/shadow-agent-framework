//! Tool system for the agent
//!
//! This module provides the Tool trait and ToolRegistry for managing
//! tools that the agent can use.

pub mod bash;
pub mod edit_tool;
pub mod glob_tool;
pub mod grep_tool;
pub mod read_tool;
mod registry;
pub mod todo;
mod tool;
pub mod write_tool;

pub use bash::BashTool;
pub use edit_tool::EditTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use read_tool::ReadTool;
pub use registry::ToolRegistry;
pub use todo::{new_todo_list, TodoItem, TodoList, TodoStatus, TodoWriteTool};
pub use tool::{Tool, ToolResult};
pub use write_tool::WriteTool;
