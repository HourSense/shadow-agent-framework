//! Useful helpers for agent implementations
//!
//! This module provides reusable components that agents can opt-in to:
//! - `TodoListManager` - Tracks tasks and which turn they were last updated

mod todo_manager;

pub use todo_manager::{TodoItem, TodoListManager, TodoStatus};
