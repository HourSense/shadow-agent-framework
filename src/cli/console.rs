use colored::*;
use std::io::{self, Write};

use crate::permissions::{PermissionDecision, PermissionRequest};
use crate::tools::{TodoItem, TodoList, TodoStatus};

/// Console handles all terminal I/O with colored formatting
pub struct Console {
    user_color: Color,
    assistant_color: Color,
    tool_color: Color,
    /// Optional shared todo list for display
    todo_list: Option<TodoList>,
}

impl Console {
    /// Create a new Console with default colors
    pub fn new() -> Self {
        Self {
            user_color: Color::Cyan,
            assistant_color: Color::Green,
            tool_color: Color::Magenta,
            todo_list: None,
        }
    }

    /// Create a new Console with a shared todo list
    pub fn with_todo_list(todo_list: TodoList) -> Self {
        Self {
            user_color: Color::Cyan,
            assistant_color: Color::Green,
            tool_color: Color::Magenta,
            todo_list: Some(todo_list),
        }
    }

    /// Create a new Console with custom colors
    pub fn with_colors(user_color: Color, assistant_color: Color, tool_color: Color) -> Self {
        Self {
            user_color,
            assistant_color,
            tool_color,
            todo_list: None,
        }
    }

    /// Set the todo list
    pub fn set_todo_list(&mut self, todo_list: TodoList) {
        self.todo_list = Some(todo_list);
    }

    /// Print a user message with colored formatting
    pub fn print_user(&self, message: &str) {
        println!("{} {}", "User:".color(self.user_color).bold(), message);
    }

    /// Print an assistant message prefix (without newline)
    pub fn print_assistant_prefix(&self) {
        print!("{} ", "Assistant:".color(self.assistant_color).bold());
        io::stdout().flush().unwrap();
    }

    /// Print a chunk of assistant response (for streaming)
    pub fn print_assistant_chunk(&self, chunk: &str) {
        print!("{}", chunk.color(self.assistant_color));
        io::stdout().flush().unwrap();
    }

    /// Print a complete assistant message with colored formatting
    pub fn print_assistant(&self, message: &str) {
        println!(
            "{} {}",
            "Assistant:".color(self.assistant_color).bold(),
            message.color(self.assistant_color)
        );
    }

    /// Print a newline
    pub fn println(&self) {
        println!();
    }

    /// Print a system message (errors, info, etc.)
    pub fn print_system(&self, message: &str) {
        println!("{} {}", "System:".yellow().bold(), message);
    }

    /// Print an error message
    pub fn print_error(&self, error: &str) {
        eprintln!("{} {}", "Error:".red().bold(), error);
    }

    /// Read a line of input from the user
    pub fn read_input(&self) -> io::Result<String> {
        print!("{} ", ">".color(self.user_color).bold());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }

    /// Print a welcome banner
    pub fn print_banner(&self) {
        println!("{}", "=".repeat(60).bright_blue());
        println!(
            "{}",
            "  Coding Agent - Powered by Claude".bright_blue().bold()
        );
        println!("{}", "=".repeat(60).bright_blue());
        println!();
        println!("Type your message and press Enter. Type 'exit' or 'quit' to end the session.");
        println!();
    }

    /// Print a separator line
    pub fn print_separator(&self) {
        println!("{}", "-".repeat(60).bright_black());
    }

    /// Print a tool action message
    pub fn print_tool_action(&self, tool_name: &str, action: &str) {
        println!(
            "{} {} {}",
            "Tool:".color(self.tool_color).bold(),
            format!("[{}]", tool_name).color(self.tool_color),
            action
        );
    }

    /// Print a tool result
    pub fn print_tool_result(&self, result: &str, is_error: bool) {
        if is_error {
            println!("{} {}", "Tool Error:".red().bold(), result);
        } else {
            // Truncate long output
            let display = if result.len() > 500 {
                format!("{}...\n(output truncated)", &result[..500])
            } else {
                result.to_string()
            };
            println!("{}", display.bright_black());
        }
    }

    /// Ask for permission to execute a tool
    ///
    /// Returns the user's decision: Allow, Deny, AlwaysAllow, or AlwaysDeny
    pub fn ask_permission(&self, request: &PermissionRequest) -> io::Result<PermissionDecision> {
        println!();
        println!("{}", "─".repeat(60).yellow());
        println!(
            "{} The agent wants to use tool: {}",
            "⚠️ Permission Required".yellow().bold(),
            request.tool_name.color(self.tool_color).bold()
        );
        println!();
        println!("  {}", request.action_description);
        if let Some(ref details) = request.details {
            println!("  {}", details.bright_black());
        }
        println!();
        println!("{}", "Options:".yellow());
        println!("  [y] Allow this action");
        println!("  [n] Deny this action");
        println!("  [a] Always allow this tool");
        println!("  [d] Always deny this tool");
        println!("{}", "─".repeat(60).yellow());
        print!("{} ", "Your choice (y/n/a/d):".yellow().bold());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        let decision = match input.as_str() {
            "y" | "yes" => PermissionDecision::Allow,
            "n" | "no" => PermissionDecision::Deny,
            "a" | "always" => PermissionDecision::AlwaysAllow,
            "d" | "deny" | "never" => PermissionDecision::AlwaysDeny,
            _ => {
                println!("{}", "Invalid choice. Defaulting to Deny.".red());
                PermissionDecision::Deny
            }
        };

        // Print confirmation
        match decision {
            PermissionDecision::Allow => {
                println!("{}", "✓ Allowed".green());
            }
            PermissionDecision::Deny => {
                println!("{}", "✗ Denied".red());
            }
            PermissionDecision::AlwaysAllow => {
                println!(
                    "{}",
                    format!("✓ Always allowing tool: {}", request.tool_name).green()
                );
            }
            PermissionDecision::AlwaysDeny => {
                println!(
                    "{}",
                    format!("✗ Always denying tool: {}", request.tool_name).red()
                );
            }
        }
        println!();

        Ok(decision)
    }

    /// Print a thinking indicator
    pub fn print_thinking(&self) {
        print!("{}", "Thinking...".bright_black());
        io::stdout().flush().unwrap();
    }

    /// Clear the thinking indicator
    pub fn clear_thinking(&self) {
        print!("\r{}\r", " ".repeat(20));
        io::stdout().flush().unwrap();
    }

    /// Print a thinking block (extended thinking content)
    pub fn print_thinking_block(&self, thinking: &str) {
        println!();
        println!("{}", "─".repeat(60).bright_blue());
        println!("{}", "💭 Agent Thinking:".bright_blue().bold());
        println!("{}", "─".repeat(60).bright_blue());

        // Display the thinking content with some formatting
        for line in thinking.lines() {
            println!("  {}", line.bright_black().italic());
        }

        println!("{}", "─".repeat(60).bright_blue());
        println!();
    }

    /// Print the todo list status
    ///
    /// Shows todos at the bottom of the console when the agent is processing.
    /// Format matches Claude Code style.
    pub fn print_todos(&self) {
        if let Some(ref todo_list) = self.todo_list {
            let todos = todo_list.read().unwrap();
            if todos.is_empty() {
                return;
            }

            println!();
            println!("{}", "─".repeat(60).bright_black());
            println!(
                "{} · {}",
                "Todos".bright_white().bold(),
                "ctrl+t to hide todos".bright_black()
            );

            for todo in todos.iter() {
                let (icon, style) = match todo.status {
                    TodoStatus::Pending => ("□", Color::BrightBlack),
                    TodoStatus::InProgress => ("◐", Color::Yellow),
                    TodoStatus::Completed => ("✓", Color::Green),
                };

                // Show activeForm for in_progress, content otherwise
                let text = if todo.status == TodoStatus::InProgress {
                    &todo.active_form
                } else {
                    &todo.content
                };

                println!("  {} {}", icon.color(style), text.color(style));
            }

            println!("{}", "─".repeat(60).bright_black());
        }
    }

    /// Print the todo list status from a given list of items
    ///
    /// Use this when you have the items directly (e.g., from TodoTracker)
    pub fn print_todos_from_items(&self, todos: &[TodoItem]) {
        if todos.is_empty() {
            return;
        }

        println!();
        println!("{}", "─".repeat(60).bright_black());
        println!(
            "{} · {}",
            "Todos".bright_white().bold(),
            "ctrl+t to hide todos".bright_black()
        );

        for todo in todos.iter() {
            let (icon, style) = match todo.status {
                TodoStatus::Pending => ("□", Color::BrightBlack),
                TodoStatus::InProgress => ("◐", Color::Yellow),
                TodoStatus::Completed => ("✓", Color::Green),
            };

            // Show activeForm for in_progress, content otherwise
            let text = if todo.status == TodoStatus::InProgress {
                &todo.active_form
            } else {
                &todo.content
            };

            println!("  {} {}", icon.color(style), text.color(style));
        }

        println!("{}", "─".repeat(60).bright_black());
    }

    /// Refresh the todo display (clear and reprint)
    pub fn refresh_todos(&self) {
        // For now, just print - in the future we could use ANSI codes to
        // update in place
        self.print_todos();
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}
