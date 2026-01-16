//! TodoWrite tool for task management
//!
//! This tool allows the agent to maintain and update a todo list
//! to track tasks it needs to perform.
//!
//! The tool looks for a `TodoListManager` in the agent's ResourceMap.
//! If found, it updates the manager; if not found, it returns an error
//! prompting the agent to ensure TodoListManager is configured.
//!
//! Usage:
//! ```ignore
//! // In agent setup:
//! internals.context.insert_resource(TodoListManager::new());
//!
//! // Register the tool (no arguments needed)
//! registry.register(TodoWriteTool::new());
//! ```

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::tool::{Tool, ToolInfo, ToolResult};
use crate::helpers::{TodoItem, TodoListManager, TodoStatus};
use crate::llm::{ToolDefinition, ToolInputSchema};
use crate::runtime::AgentInternals;

/// Input for the todo tool (matches the LLM schema)
#[derive(Debug, Deserialize)]
struct TodoInput {
    /// The full list of todos to set
    todos: Vec<TodoItemInput>,
}

/// Input format for a single todo item from the LLM
#[derive(Debug, Deserialize)]
struct TodoItemInput {
    content: String,
    status: String,
    #[serde(rename = "activeForm")]
    active_form: String,
}

impl TodoItemInput {
    /// Convert to the helpers::TodoItem type
    fn into_todo_item(self) -> TodoItem {
        let status = match self.status.as_str() {
            "in_progress" => TodoStatus::InProgress,
            "completed" => TodoStatus::Completed,
            _ => TodoStatus::Pending,
        };
        TodoItem::with_status(self.content, self.active_form, status)
    }
}

/// TodoWrite tool for managing tasks
///
/// This tool reads from and writes to a `TodoListManager` stored in the
/// agent's ResourceMap. The manager must be added to the context before
/// the tool can be used.
pub struct TodoWriteTool;

impl TodoWriteTool {
    /// Create a new TodoWrite tool
    ///
    /// The tool will look for TodoListManager in the agent's resources.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }

    fn description(&self) -> &str {
        "Create and manage a structured task list to track progress and organize tasks."
    }

    fn definition(&self) -> ToolDefinition {
        use crate::llm::types::CustomTool;

        ToolDefinition::Custom(CustomTool {
            name: "TodoWrite".to_string(),
            description: Some(
                "Use this tool to create and manage a structured task list for the current session. \
                This helps track progress and organize complex tasks. \
                Each todo has content (what to do), status (pending/in_progress/completed), \
                and activeForm (present continuous description)."
                    .to_string(),
            ),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: Some(json!({
                    "todos": {
                        "type": "array",
                        "description": "The updated todo list",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "minLength": 1,
                                    "description": "The imperative form describing what needs to be done (e.g., 'Run tests')"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Current status of the task"
                                },
                                "activeForm": {
                                    "type": "string",
                                    "minLength": 1,
                                    "description": "The present continuous form shown during execution (e.g., 'Running tests')"
                                }
                            },
                            "required": ["content", "status", "activeForm"]
                        }
                    }
                })),
                required: Some(vec!["todos".to_string()]),
            },
            tool_type: None,
        })
    }

    fn get_info(&self, input: &Value) -> ToolInfo {
        let todo_count = input
            .get("todos")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        ToolInfo {
            name: "TodoWrite".to_string(),
            action_description: format!("Update todo list ({} items)", todo_count),
            details: None,
        }
    }

    async fn execute(&self, input: &Value, internals: &mut AgentInternals) -> Result<ToolResult> {
        // Parse the input
        let todo_input: TodoInput = serde_json::from_value(input.clone())
            .map_err(|e| anyhow::anyhow!("Invalid todo input: {}", e))?;

        // Look for TodoListManager in resources
        let manager = match internals.context.get_resource::<TodoListManager>() {
            Some(m) => m,
            None => {
                return Ok(ToolResult::error(
                    "TodoListManager not found in agent resources. \
                    Ensure TodoListManager is added to context before using TodoWriteTool."
                ));
            }
        };

        // Convert input items to TodoItem
        let items: Vec<TodoItem> = todo_input
            .todos
            .into_iter()
            .map(|i| i.into_todo_item())
            .collect();

        // Get current turn from context
        let current_turn = internals.context.current_turn;

        // Update the manager
        manager.set_todos(items, current_turn);

        // Return the formatted list
        let output = manager.format();
        Ok(ToolResult::success(output))
    }

    fn requires_permission(&self) -> bool {
        false // Todo updates don't need permission
    }
}
