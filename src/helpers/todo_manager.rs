//! Todo List Manager
//!
//! A helper that manages a todo list for agents. This is stored in the
//! ResourceMap and accessed by the TodoWriteTool through AgentInternals.
//!
//! Usage:
//! ```ignore
//! // In agent setup, add to context resources:
//! internals.context.insert_resource(TodoListManager::new());
//!
//! // TodoWriteTool will automatically find and update it
//! ```

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// Status of a todo item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoStatus::Pending => write!(f, "pending"),
            TodoStatus::InProgress => write!(f, "in_progress"),
            TodoStatus::Completed => write!(f, "completed"),
        }
    }
}

/// A single todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// The imperative form describing what needs to be done
    pub content: String,
    /// Current status of the task
    pub status: TodoStatus,
    /// The present continuous form shown during execution (e.g., "Running tests")
    #[serde(rename = "activeForm")]
    pub active_form: String,
}

impl TodoItem {
    /// Create a new pending todo item
    pub fn new(content: impl Into<String>, active_form: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            status: TodoStatus::Pending,
            active_form: active_form.into(),
        }
    }

    /// Create a todo item with a specific status
    pub fn with_status(
        content: impl Into<String>,
        active_form: impl Into<String>,
        status: TodoStatus,
    ) -> Self {
        Self {
            content: content.into(),
            status,
            active_form: active_form.into(),
        }
    }
}

/// Internal state protected by RwLock
struct TodoListState {
    /// The list of todo items
    items: Vec<TodoItem>,
    /// The turn number when todos were last updated
    last_updated_turn: usize,
}

/// Manager for agent todo lists
///
/// This is stored in the agent's ResourceMap and accessed by TodoWriteTool.
/// It tracks both the todo list and when it was last updated.
pub struct TodoListManager {
    state: RwLock<TodoListState>,
}

impl TodoListManager {
    /// Create a new empty todo list manager
    pub fn new() -> Self {
        Self {
            state: RwLock::new(TodoListState {
                items: Vec::new(),
                last_updated_turn: 0,
            }),
        }
    }

    /// Get the current todo list
    pub fn get_todos(&self) -> Vec<TodoItem> {
        self.state.read().unwrap().items.clone()
    }

    /// Set the todo list and update the turn number
    pub fn set_todos(&self, items: Vec<TodoItem>, turn: usize) {
        let mut state = self.state.write().unwrap();
        state.items = items;
        state.last_updated_turn = turn;
    }

    /// Get the turn number when todos were last updated
    pub fn last_updated_turn(&self) -> usize {
        self.state.read().unwrap().last_updated_turn
    }

    /// Check if the todo list is empty
    pub fn is_empty(&self) -> bool {
        self.state.read().unwrap().items.is_empty()
    }

    /// Get the number of todo items
    pub fn len(&self) -> usize {
        self.state.read().unwrap().items.len()
    }

    /// Get counts by status
    pub fn counts(&self) -> (usize, usize, usize) {
        let state = self.state.read().unwrap();
        let pending = state
            .items
            .iter()
            .filter(|t| t.status == TodoStatus::Pending)
            .count();
        let in_progress = state
            .items
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();
        let completed = state
            .items
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        (pending, in_progress, completed)
    }

    /// Get the currently in-progress task (if any)
    pub fn current_task(&self) -> Option<TodoItem> {
        self.state
            .read()
            .unwrap()
            .items
            .iter()
            .find(|t| t.status == TodoStatus::InProgress)
            .cloned()
    }

    /// Format the todo list for display
    pub fn format(&self) -> String {
        let state = self.state.read().unwrap();

        if state.items.is_empty() {
            return "No tasks in the todo list.".to_string();
        }

        let mut output = String::new();
        output.push_str("Todo List:\n");

        for (i, item) in state.items.iter().enumerate() {
            let status_icon = match item.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[*]",
                TodoStatus::Completed => "[x]",
            };
            output.push_str(&format!("  {} {}. {}\n", status_icon, i + 1, item.content));
        }

        // Show summary
        let (pending, in_progress, completed) = drop_and_count(&state.items);
        output.push_str(&format!(
            "\nSummary: {} pending, {} in progress, {} completed\n",
            pending, in_progress, completed
        ));

        output
    }
}

impl Default for TodoListManager {
    fn default() -> Self {
        Self::new()
    }
}

// Helper to avoid holding the lock while counting
fn drop_and_count(items: &[TodoItem]) -> (usize, usize, usize) {
    let pending = items.iter().filter(|t| t.status == TodoStatus::Pending).count();
    let in_progress = items
        .iter()
        .filter(|t| t.status == TodoStatus::InProgress)
        .count();
    let completed = items
        .iter()
        .filter(|t| t.status == TodoStatus::Completed)
        .count();
    (pending, in_progress, completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager() {
        let manager = TodoListManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
        assert_eq!(manager.last_updated_turn(), 0);
    }

    #[test]
    fn test_set_and_get_todos() {
        let manager = TodoListManager::new();

        let todos = vec![
            TodoItem::new("First task", "Working on first task"),
            TodoItem::with_status("Second task", "Working on second task", TodoStatus::Completed),
        ];

        manager.set_todos(todos.clone(), 5);

        assert_eq!(manager.len(), 2);
        assert_eq!(manager.last_updated_turn(), 5);

        let retrieved = manager.get_todos();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].content, "First task");
        assert_eq!(retrieved[1].status, TodoStatus::Completed);
    }

    #[test]
    fn test_counts() {
        let manager = TodoListManager::new();

        let todos = vec![
            TodoItem::new("Task 1", "Working"),
            TodoItem::with_status("Task 2", "Working", TodoStatus::InProgress),
            TodoItem::with_status("Task 3", "Working", TodoStatus::Completed),
            TodoItem::with_status("Task 4", "Working", TodoStatus::Completed),
        ];

        manager.set_todos(todos, 1);

        let (pending, in_progress, completed) = manager.counts();
        assert_eq!(pending, 1);
        assert_eq!(in_progress, 1);
        assert_eq!(completed, 2);
    }

    #[test]
    fn test_current_task() {
        let manager = TodoListManager::new();

        // No in-progress task
        assert!(manager.current_task().is_none());

        let todos = vec![
            TodoItem::new("Task 1", "Working"),
            TodoItem::with_status("Task 2", "Working on task 2", TodoStatus::InProgress),
        ];

        manager.set_todos(todos, 1);

        let current = manager.current_task();
        assert!(current.is_some());
        assert_eq!(current.unwrap().content, "Task 2");
    }
}
