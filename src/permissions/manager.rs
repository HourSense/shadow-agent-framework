//! Permission manager implementation
//!
//! Provides a three-tier permission system:
//! - Global: Shared across all agents (Arc<RwLock<>>)
//! - Local: Agent-type specific rules
//! - Session: Rules added during current session

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Type of permission rule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    /// Allow the entire tool (e.g., Read is always allowed)
    AllowTool,
    /// Allow commands starting with a specific prefix
    AllowPrefix,
}

/// A permission rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Type of rule
    pub rule_type: RuleType,
    /// Tool name (mandatory)
    pub tool_name: String,
    /// Prefix for AllowPrefix rules (e.g., "cd", "git status")
    pub prefix: Option<String>,
}

impl PermissionRule {
    /// Create a rule that allows an entire tool
    pub fn allow_tool(tool_name: impl Into<String>) -> Self {
        Self {
            rule_type: RuleType::AllowTool,
            tool_name: tool_name.into(),
            prefix: None,
        }
    }

    /// Create a rule that allows commands with a specific prefix
    pub fn allow_prefix(tool_name: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self {
            rule_type: RuleType::AllowPrefix,
            tool_name: tool_name.into(),
            prefix: Some(prefix.into()),
        }
    }

    /// Check if this rule matches the given tool and input
    pub fn matches(&self, tool_name: &str, input: &str) -> bool {
        if self.tool_name != tool_name {
            return false;
        }

        match self.rule_type {
            RuleType::AllowTool => true,
            RuleType::AllowPrefix => {
                if let Some(prefix) = &self.prefix {
                    input.trim_start().starts_with(prefix)
                } else {
                    false
                }
            }
        }
    }
}

/// Result of checking permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckResult {
    /// Tool/action is allowed by a rule
    Allowed,
    /// Need to ask user for permission
    AskUser,
    /// Denied (non-interactive mode, no matching rule)
    Denied,
}

/// Scope for where to store a permission rule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionScope {
    /// Just this session (in-memory)
    Session,
    /// This agent type (persisted)
    Local,
    /// All agents (persisted, shared)
    Global,
}

/// A request for permission to execute a tool
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// Name of the tool
    pub tool_name: String,
    /// Human-readable description of the action
    pub action_description: String,
    /// The actual input/command (for prefix matching)
    pub input: String,
    /// Optional additional details
    pub details: Option<String>,
}

impl PermissionRequest {
    /// Create a new permission request
    pub fn new(
        tool_name: impl Into<String>,
        action_description: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            action_description: action_description.into(),
            input: input.into(),
            details: None,
        }
    }

    /// Add details to the request
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// The user's decision on a permission request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    /// Allow this action (one-time)
    Allow,
    /// Deny this action (one-time)
    Deny,
    /// Always allow this tool/action
    AlwaysAllow,
    /// Always deny this tool/action (rarely used)
    AlwaysDeny,
}

/// Global permissions shared across all agents
///
/// This is wrapped in Arc<RwLock<>> and passed to all agents.
/// When a user says "allow for all agents", rules are added here
/// and immediately visible to all running agents.
#[derive(Debug, Default)]
pub struct GlobalPermissions {
    rules: RwLock<Vec<PermissionRule>>,
}

impl GlobalPermissions {
    /// Create new global permissions
    pub fn new() -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
        }
    }

    /// Create with initial rules
    pub fn with_rules(rules: Vec<PermissionRule>) -> Self {
        Self {
            rules: RwLock::new(rules),
        }
    }

    /// Add a rule
    pub fn add_rule(&self, rule: PermissionRule) {
        let mut rules = self.rules.write().unwrap();
        // Avoid duplicates
        if !rules.iter().any(|r| {
            r.tool_name == rule.tool_name
                && r.rule_type == rule.rule_type
                && r.prefix == rule.prefix
        }) {
            tracing::info!(
                "Adding global permission rule: {:?} for {}",
                rule.rule_type,
                rule.tool_name
            );
            rules.push(rule);
        }
    }

    /// Check if any rule matches
    pub fn check(&self, tool_name: &str, input: &str) -> bool {
        let rules = self.rules.read().unwrap();
        rules.iter().any(|r| r.matches(tool_name, input))
    }

    /// Get all rules (for persistence)
    pub fn rules(&self) -> Vec<PermissionRule> {
        self.rules.read().unwrap().clone()
    }

    /// Clear all rules
    pub fn clear(&self) {
        self.rules.write().unwrap().clear();
    }
}

/// Per-agent permission manager
///
/// Each agent has its own PermissionManager that references:
/// - Shared global permissions (Arc)
/// - Local rules (agent-type specific)
/// - Session rules (this session only)
pub struct PermissionManager {
    /// Shared global permissions
    global: Arc<GlobalPermissions>,
    /// Agent-type specific rules
    local: Vec<PermissionRule>,
    /// Rules added during this session
    session: Vec<PermissionRule>,
    /// Whether we can prompt the user (false for background agents)
    interactive: bool,
    /// Agent type (for loading/saving local rules)
    agent_type: String,
}

impl PermissionManager {
    /// Create a new permission manager
    pub fn new(global: Arc<GlobalPermissions>, agent_type: impl Into<String>) -> Self {
        Self {
            global,
            local: Vec::new(),
            session: Vec::new(),
            interactive: true,
            agent_type: agent_type.into(),
        }
    }

    /// Create with local rules
    pub fn with_local_rules(
        global: Arc<GlobalPermissions>,
        agent_type: impl Into<String>,
        local_rules: Vec<PermissionRule>,
    ) -> Self {
        Self {
            global,
            local: local_rules,
            session: Vec::new(),
            interactive: true,
            agent_type: agent_type.into(),
        }
    }

    /// Set interactive mode
    pub fn set_interactive(&mut self, interactive: bool) {
        self.interactive = interactive;
    }

    /// Check if a tool action is allowed
    ///
    /// Checks in order: session → local → global
    /// Returns Allowed if any rule matches, otherwise AskUser (or Denied if non-interactive)
    pub fn check(&self, tool_name: &str, input: &str) -> CheckResult {
        // Check session rules first
        if self.session.iter().any(|r| r.matches(tool_name, input)) {
            return CheckResult::Allowed;
        }

        // Check local (agent-type) rules
        if self.local.iter().any(|r| r.matches(tool_name, input)) {
            return CheckResult::Allowed;
        }

        // Check global rules
        if self.global.check(tool_name, input) {
            return CheckResult::Allowed;
        }

        // No matching rule
        if self.interactive {
            CheckResult::AskUser
        } else {
            CheckResult::Denied
        }
    }

    /// Add a rule at the specified scope
    pub fn add_rule(&mut self, rule: PermissionRule, scope: PermissionScope) {
        match scope {
            PermissionScope::Session => {
                tracing::info!(
                    "Adding session permission rule: {:?} for {}",
                    rule.rule_type,
                    rule.tool_name
                );
                self.session.push(rule);
            }
            PermissionScope::Local => {
                tracing::info!(
                    "Adding local permission rule: {:?} for {} (agent: {})",
                    rule.rule_type,
                    rule.tool_name,
                    self.agent_type
                );
                self.local.push(rule);
            }
            PermissionScope::Global => {
                self.global.add_rule(rule);
            }
        }
    }

    /// Process a permission decision
    ///
    /// If the decision is AlwaysAllow, creates and stores a rule.
    /// Returns whether the action should be allowed.
    pub fn process_decision(
        &mut self,
        tool_name: &str,
        _input: &str,
        decision: PermissionDecision,
        scope: PermissionScope,
    ) -> bool {
        match decision {
            PermissionDecision::Allow => true,
            PermissionDecision::Deny => false,
            PermissionDecision::AlwaysAllow => {
                // Create appropriate rule based on input
                // For now, just allow the whole tool
                let rule = PermissionRule::allow_tool(tool_name);
                self.add_rule(rule, scope);
                true
            }
            PermissionDecision::AlwaysDeny => {
                // We don't typically use deny rules, but support it
                tracing::warn!(
                    "AlwaysDeny requested for {} - not creating rule (ask-based model)",
                    tool_name
                );
                false
            }
        }
    }

    /// Get all session rules
    pub fn session_rules(&self) -> &[PermissionRule] {
        &self.session
    }

    /// Get all local rules
    pub fn local_rules(&self) -> &[PermissionRule] {
        &self.local
    }

    /// Get the global permissions reference
    pub fn global(&self) -> &Arc<GlobalPermissions> {
        &self.global
    }

    /// Get the agent type
    pub fn agent_type(&self) -> &str {
        &self.agent_type
    }

    /// Clear session rules
    pub fn clear_session_rules(&mut self) {
        self.session.clear();
    }

    /// Check if running in interactive mode
    pub fn is_interactive(&self) -> bool {
        self.interactive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_allow_tool() {
        let rule = PermissionRule::allow_tool("Read");

        assert!(rule.matches("Read", "any input"));
        assert!(rule.matches("Read", ""));
        assert!(!rule.matches("Write", "any input"));
    }

    #[test]
    fn test_rule_allow_prefix() {
        let rule = PermissionRule::allow_prefix("Bash", "cd");

        assert!(rule.matches("Bash", "cd /home"));
        assert!(rule.matches("Bash", "cd"));
        assert!(rule.matches("Bash", "  cd /home")); // trim_start
        assert!(!rule.matches("Bash", "rm -rf"));
        assert!(!rule.matches("Write", "cd /home"));
    }

    #[test]
    fn test_global_permissions() {
        let global = GlobalPermissions::new();
        global.add_rule(PermissionRule::allow_tool("Read"));
        global.add_rule(PermissionRule::allow_prefix("Bash", "git status"));

        assert!(global.check("Read", "anything"));
        assert!(global.check("Bash", "git status"));
        assert!(!global.check("Bash", "rm -rf /"));
        assert!(!global.check("Write", "file.txt"));
    }

    #[test]
    fn test_permission_manager_hierarchy() {
        let global = Arc::new(GlobalPermissions::new());
        global.add_rule(PermissionRule::allow_tool("Read"));

        let mut manager = PermissionManager::new(global, "test-agent");
        manager.local.push(PermissionRule::allow_tool("Grep"));
        manager.session.push(PermissionRule::allow_prefix("Bash", "ls"));

        // Session rule
        assert_eq!(manager.check("Bash", "ls -la"), CheckResult::Allowed);

        // Local rule
        assert_eq!(manager.check("Grep", "pattern"), CheckResult::Allowed);

        // Global rule
        assert_eq!(manager.check("Read", "file.txt"), CheckResult::Allowed);

        // No rule - ask user
        assert_eq!(manager.check("Write", "file.txt"), CheckResult::AskUser);
    }

    #[test]
    fn test_non_interactive_denies() {
        let global = Arc::new(GlobalPermissions::new());
        let mut manager = PermissionManager::new(global, "test-agent");
        manager.set_interactive(false);

        assert_eq!(manager.check("Bash", "rm -rf"), CheckResult::Denied);
    }

    #[test]
    fn test_process_decision_always_allow() {
        let global = Arc::new(GlobalPermissions::new());
        let mut manager = PermissionManager::new(global.clone(), "test-agent");

        // Process always allow at session scope
        let allowed = manager.process_decision(
            "Write",
            "file.txt",
            PermissionDecision::AlwaysAllow,
            PermissionScope::Session,
        );
        assert!(allowed);
        assert_eq!(manager.check("Write", "anything"), CheckResult::Allowed);

        // Process always allow at global scope
        let mut manager2 = PermissionManager::new(global.clone(), "other-agent");
        let allowed = manager2.process_decision(
            "Bash",
            "echo",
            PermissionDecision::AlwaysAllow,
            PermissionScope::Global,
        );
        assert!(allowed);

        // Both managers should see the global rule
        assert_eq!(manager.check("Bash", "echo hi"), CheckResult::Allowed);
        assert_eq!(manager2.check("Bash", "echo hi"), CheckResult::Allowed);
    }

    #[test]
    fn test_global_shared_across_managers() {
        let global = Arc::new(GlobalPermissions::new());

        let manager1 = PermissionManager::new(global.clone(), "agent1");
        let manager2 = PermissionManager::new(global.clone(), "agent2");

        // Add rule via global directly
        global.add_rule(PermissionRule::allow_tool("Read"));

        // Both managers should see it immediately
        assert_eq!(manager1.check("Read", "file"), CheckResult::Allowed);
        assert_eq!(manager2.check("Read", "file"), CheckResult::Allowed);
    }
}
