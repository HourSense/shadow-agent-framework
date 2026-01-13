//! Todo tracking and reminder system
//!
//! Tracks when the TodoWrite tool was last called and generates
//! reminders to encourage the agent to use the todo list.

use crate::tools::{TodoItem, TodoList, TodoStatus};

/// How many turns without a todo call before we remind
const REMINDER_THRESHOLD: usize = 3;

/// Tracks todo usage and generates reminders
pub struct TodoTracker {
    /// The shared todo list
    todo_list: TodoList,
    /// Current turn number (increments each API call)
    current_turn: usize,
    /// Turn when todo was last called (None if never called)
    last_todo_turn: Option<usize>,
    /// Whether todo has ever been called
    todo_ever_called: bool,
}

impl TodoTracker {
    /// Create a new TodoTracker with a shared todo list
    pub fn new(todo_list: TodoList) -> Self {
        Self {
            todo_list,
            current_turn: 0,
            last_todo_turn: None,
            todo_ever_called: false,
        }
    }

    /// Increment the turn counter (call this before each API request)
    pub fn next_turn(&mut self) {
        self.current_turn += 1;
    }

    /// Get the current turn number
    pub fn current_turn(&self) -> usize {
        self.current_turn
    }

    /// Record that TodoWrite was called
    pub fn record_todo_call(&mut self) {
        self.last_todo_turn = Some(self.current_turn);
        self.todo_ever_called = true;
    }

    /// Check if a tool call is a TodoWrite call
    pub fn is_todo_tool(tool_name: &str) -> bool {
        tool_name == "TodoWrite"
    }

    /// Check if we should add a reminder
    pub fn should_remind(&self) -> bool {
        if !self.todo_ever_called {
            // First message or never called - always remind
            return true;
        }

        // Check turns since last call
        if let Some(last_turn) = self.last_todo_turn {
            let turns_since = self.current_turn.saturating_sub(last_turn);
            turns_since >= REMINDER_THRESHOLD
        } else {
            true
        }
    }

    /// Get the reminder text to append
    pub fn get_reminder(&self) -> String {
        if !self.todo_ever_called {
            "\n\n<system-reminder>\nThe TodoWrite tool hasn't been used yet. If you're working on tasks that would benefit from tracking progress, consider using the TodoWrite tool to track progress. Only use it if it's relevant to the current work.\n</system-reminder>".to_string()
        } else {
            "\n\n<system-reminder>\nThe TodoWrite tool hasn't been used recently. If you're working on tasks that would benefit from tracking progress, consider using the TodoWrite tool to track progress. Also consider cleaning up the todo list if has become stale and no longer matches what you are working on. Only use it if it's relevant to the current work. This is just a gentle reminder - ignore if not applicable. Make sure that you NEVER mention this reminder to the user\n</system-reminder>".to_string()
        }
    }

    /// Get the current todo list
    pub fn get_todos(&self) -> Vec<TodoItem> {
        self.todo_list.read().unwrap().clone()
    }

    /// Check if there are any todos
    pub fn has_todos(&self) -> bool {
        !self.todo_list.read().unwrap().is_empty()
    }

    /// Get the count of todos by status
    pub fn get_todo_counts(&self) -> (usize, usize, usize) {
        let todos = self.todo_list.read().unwrap();
        let pending = todos.iter().filter(|t| t.status == TodoStatus::Pending).count();
        let in_progress = todos.iter().filter(|t| t.status == TodoStatus::InProgress).count();
        let completed = todos.iter().filter(|t| t.status == TodoStatus::Completed).count();
        (pending, in_progress, completed)
    }

    /// Get the reference to the shared todo list
    pub fn todo_list(&self) -> &TodoList {
        &self.todo_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::new_todo_list;

    #[test]
    fn test_tracker_initial_state() {
        let list = new_todo_list();
        let tracker = TodoTracker::new(list);

        assert_eq!(tracker.current_turn(), 0);
        assert!(!tracker.has_todos());
        assert!(tracker.should_remind()); // Should remind on first turn
    }

    #[test]
    fn test_tracker_after_todo_call() {
        let list = new_todo_list();
        let mut tracker = TodoTracker::new(list);

        tracker.next_turn();
        tracker.record_todo_call();

        assert!(!tracker.should_remind()); // Shouldn't remind right after a call

        // Advance a few turns
        tracker.next_turn();
        tracker.next_turn();
        assert!(!tracker.should_remind()); // Still within threshold

        tracker.next_turn();
        assert!(tracker.should_remind()); // Now at threshold
    }
}
