//! Agent Configuration
//!
//! Configuration options for the StandardAgent.

use std::sync::Arc;

use crate::helpers::InjectionChain;
use crate::hooks::HookRegistry;
use crate::llm::ThinkingConfig;
use crate::tools::ToolRegistry;

/// Configuration for a StandardAgent
///
/// Use the builder pattern to configure the agent:
///
/// ```ignore
/// let config = AgentConfig::new("You are a helpful assistant")
///     .with_tools(tools)
///     .with_injection_chain(injections)
///     .with_max_tool_iterations(50)
///     .with_thinking(16000)  // Enable extended thinking with 16k token budget
///     .with_streaming(true)
///     .with_debug(true);
/// ```
pub struct AgentConfig {
    /// System prompt for the LLM
    pub system_prompt: String,

    /// Tool registry (optional - agent can work without tools)
    pub tools: Option<Arc<ToolRegistry>>,

    /// Context injection chain (applied before each LLM call)
    pub injections: InjectionChain,

    /// Maximum number of tool iterations per turn (prevents infinite loops)
    pub max_tool_iterations: usize,

    /// Whether to auto-save session after each turn
    pub auto_save_session: bool,

    /// Whether to enable debug logging (API calls, tool calls)
    pub debug_enabled: bool,

    /// Whether to enable streaming responses from the LLM
    pub streaming_enabled: bool,

    /// Extended thinking configuration (optional)
    /// When enabled, Claude will show its step-by-step reasoning process.
    pub thinking: Option<ThinkingConfig>,

    /// Hooks for intercepting agent behavior
    /// Use hooks to block dangerous operations, modify tool inputs, auto-approve tools, etc.
    pub hooks: Option<Arc<HookRegistry>>,
}

impl AgentConfig {
    /// Create a new agent configuration with a system prompt
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            tools: None,
            injections: InjectionChain::new(),
            max_tool_iterations: 100,
            auto_save_session: true,
            debug_enabled: false,
            streaming_enabled: false,
            thinking: None,
            hooks: None,
        }
    }

    /// Set the tool registry
    pub fn with_tools(mut self, tools: Arc<ToolRegistry>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set the context injection chain
    pub fn with_injection_chain(mut self, injections: InjectionChain) -> Self {
        self.injections = injections;
        self
    }

    /// Add a single injection to the chain
    pub fn with_injection<I: crate::helpers::ContextInjection + 'static>(
        mut self,
        injection: I,
    ) -> Self {
        self.injections.add(injection);
        self
    }

    /// Add a function-based injection
    pub fn with_injection_fn<F>(mut self, name: impl Into<String>, func: F) -> Self
    where
        F: Fn(&crate::runtime::AgentInternals, Vec<crate::llm::Message>) -> Vec<crate::llm::Message>
            + Send
            + Sync
            + 'static,
    {
        self.injections.add_fn(name, func);
        self
    }

    /// Set maximum tool iterations per turn
    pub fn with_max_tool_iterations(mut self, max: usize) -> Self {
        self.max_tool_iterations = max;
        self
    }

    /// Set whether to auto-save session after each turn
    pub fn with_auto_save(mut self, auto_save: bool) -> Self {
        self.auto_save_session = auto_save;
        self
    }

    /// Enable or disable debug logging
    ///
    /// When enabled, the agent will log all API requests/responses and tool
    /// calls to a `debugger/` folder in the session directory.
    pub fn with_debug(mut self, enabled: bool) -> Self {
        self.debug_enabled = enabled;
        self
    }

    /// Enable or disable streaming responses
    ///
    /// When enabled, the agent will stream LLM responses in real-time,
    /// sending text deltas as they arrive. This provides a better user
    /// experience for interactive applications.
    pub fn with_streaming(mut self, enabled: bool) -> Self {
        self.streaming_enabled = enabled;
        self
    }

    /// Enable extended thinking with a token budget
    ///
    /// Extended thinking gives Claude enhanced reasoning capabilities for complex tasks.
    /// Claude will show its step-by-step thought process before delivering its final answer.
    ///
    /// The `budget_tokens` parameter determines the maximum tokens Claude can use for thinking.
    /// Minimum is 1024 tokens. Larger budgets can improve response quality for complex problems.
    ///
    /// Note: When thinking is enabled, temperature is automatically set to 1 (required by API).
    pub fn with_thinking(mut self, budget_tokens: u32) -> Self {
        self.thinking = Some(ThinkingConfig::enabled(budget_tokens));
        self
    }

    /// Set a custom thinking configuration
    pub fn with_thinking_config(mut self, config: ThinkingConfig) -> Self {
        self.thinking = Some(config);
        self
    }

    /// Set the hook registry for intercepting agent behavior
    ///
    /// Hooks allow you to:
    /// - Block dangerous operations before they execute
    /// - Modify tool arguments (e.g., rewrite paths for remote filesystem)
    /// - Auto-approve certain tools
    /// - Log and audit tool calls
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut hooks = HookRegistry::new();
    /// hooks.add_with_pattern(HookEvent::PreToolUse, "Bash", |ctx| {
    ///     if ctx.tool_input.as_ref()
    ///         .and_then(|v| v.get("command"))
    ///         .and_then(|v| v.as_str())
    ///         .map(|c| c.contains("rm -rf"))
    ///         .unwrap_or(false)
    ///     {
    ///         HookResult::deny("Dangerous command blocked")
    ///     } else {
    ///         HookResult::none()
    ///     }
    /// })?;
    ///
    /// let config = AgentConfig::new("...").with_hooks(hooks);
    /// ```
    pub fn with_hooks(mut self, hooks: HookRegistry) -> Self {
        self.hooks = Some(Arc::new(hooks));
        self
    }

    /// Get tool definitions (empty vec if no tools)
    pub fn tool_definitions(&self) -> Vec<crate::llm::ToolDefinition> {
        self.tools
            .as_ref()
            .map(|t| t.get_definitions())
            .unwrap_or_default()
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self::new("You are a helpful assistant.")
    }
}

impl std::fmt::Debug for AgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConfig")
            .field("system_prompt", &format!("{}...", &self.system_prompt.chars().take(50).collect::<String>()))
            .field("tools", &self.tools.as_ref().map(|t| t.tool_names()))
            .field("max_tool_iterations", &self.max_tool_iterations)
            .field("auto_save_session", &self.auto_save_session)
            .field("debug_enabled", &self.debug_enabled)
            .field("streaming_enabled", &self.streaming_enabled)
            .field("thinking", &self.thinking)
            .field("hooks", &self.hooks.as_ref().map(|h| format!("{:?}", h)))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_defaults() {
        let config = AgentConfig::default();
        assert!(!config.debug_enabled);
        assert!(config.auto_save_session);
        assert_eq!(config.max_tool_iterations, 100);
    }

    #[test]
    fn test_agent_config_with_debug() {
        let config = AgentConfig::new("Test").with_debug(true);
        assert!(config.debug_enabled);
    }
}
