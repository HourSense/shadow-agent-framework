use singapore_project::agent::{Agent, default_system_prompt};
use singapore_project::cli::Console;
use singapore_project::context::ContextManager;
use singapore_project::debugger::Debugger;
use singapore_project::llm::AnthropicProvider;
use singapore_project::logging;
use singapore_project::tools::{
    new_todo_list, BashTool, EditTool, GlobTool, GrepTool, ReadTool, TodoWriteTool, ToolRegistry,
    WriteTool,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging system
    logging::init_logging()?;

    tracing::info!("=== Coding Agent Starting ===");

    // Create shared todo list first (shared between console, tool, and agent)
    let todo_list = new_todo_list();

    // Create console for terminal I/O with todo list
    let console = Console::with_todo_list(todo_list.clone());

    // Create Anthropic LLM provider from environment
    let llm_provider = AnthropicProvider::from_env()?;

    // Create tool registry with available tools
    let mut tool_registry = ToolRegistry::new();

    // Core tools
    tool_registry.register(BashTool::new()?);

    // File operations (split tools)
    tool_registry.register(ReadTool::new()?);
    tool_registry.register(EditTool::new()?);
    tool_registry.register(WriteTool::new()?);

    // Search tools
    tool_registry.register(GlobTool::new()?);
    tool_registry.register(GrepTool::new()?);

    // Task management - uses the same shared todo list
    tool_registry.register(TodoWriteTool::new(todo_list.clone()));

    tracing::info!("Registered {} tools", tool_registry.len());

    // Create context manager with system prompt
    let context_manager = ContextManager::new(default_system_prompt());

    // Create debugger for logging all requests/responses
    let debugger = Debugger::new()?;
    tracing::info!("Debugger session: {:?}", debugger.session_dir());

    // Create agent with all components - pass todo_list for tracker
    let mut agent = Agent::new(
        console,
        llm_provider,
        tool_registry,
        context_manager,
        debugger,
        todo_list,
    )?;

    tracing::info!(
        "Agent initialized with conversation ID: {}",
        agent.conversation_id()
    );

    // Run the agent loop
    agent.run().await?;

    tracing::info!("=== Coding Agent Shutting Down ===");

    Ok(())
}
