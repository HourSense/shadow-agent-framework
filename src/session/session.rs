//! Agent session management
//!
//! The `AgentSession` struct combines metadata and message history,
//! providing a complete view of an agent's conversation state.

use crate::core::FrameworkResult;
use crate::llm::Message;

use super::metadata::SessionMetadata;
use super::storage::SessionStorage;

/// An agent session that tracks conversation history and metadata
///
/// Each agent has its own session, identified by a unique session_id.
/// Sessions can be linked via parent/child relationships for subagent tracking.
#[derive(Debug)]
pub struct AgentSession {
    /// Session metadata (identity, lineage, timestamps)
    pub metadata: SessionMetadata,

    /// Conversation history
    pub messages: Vec<Message>,

    /// Storage backend for persistence
    storage: SessionStorage,
}

impl AgentSession {
    /// Create a new root agent session
    ///
    /// This creates a new session with the given metadata and empty history.
    /// The session is automatically persisted to disk.
    pub fn new(
        session_id: impl Into<String>,
        agent_type: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> FrameworkResult<Self> {
        let storage = SessionStorage::new();
        let metadata = SessionMetadata::new(session_id, agent_type, name, description);

        // Persist the metadata
        storage.save_metadata(&metadata)?;

        Ok(Self {
            metadata,
            messages: Vec::new(),
            storage,
        })
    }

    /// Create a new root agent session with custom storage
    pub fn new_with_storage(
        session_id: impl Into<String>,
        agent_type: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        storage: SessionStorage,
    ) -> FrameworkResult<Self> {
        let metadata = SessionMetadata::new(session_id, agent_type, name, description);

        // Persist the metadata
        storage.save_metadata(&metadata)?;

        Ok(Self {
            metadata,
            messages: Vec::new(),
            storage,
        })
    }

    /// Create a new subagent session
    ///
    /// This creates a session that is linked to a parent session.
    /// The parent session is automatically updated to track this child.
    pub fn new_subagent(
        session_id: impl Into<String>,
        agent_type: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        parent_session_id: impl Into<String>,
        parent_tool_use_id: impl Into<String>,
    ) -> FrameworkResult<Self> {
        let storage = SessionStorage::new();
        let parent_id = parent_session_id.into();

        let metadata = SessionMetadata::new_subagent(
            session_id,
            agent_type,
            name,
            description,
            &parent_id,
            parent_tool_use_id,
        );

        // Persist the metadata
        storage.save_metadata(&metadata)?;

        // Update parent to track this child
        if let Ok(mut parent_meta) = storage.load_metadata(&parent_id) {
            parent_meta.add_child(&metadata.session_id);
            storage.save_metadata(&parent_meta)?;
        }

        Ok(Self {
            metadata,
            messages: Vec::new(),
            storage,
        })
    }

    /// Create a new subagent session with custom storage
    pub fn new_subagent_with_storage(
        session_id: impl Into<String>,
        agent_type: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        parent_session_id: impl Into<String>,
        parent_tool_use_id: impl Into<String>,
        storage: SessionStorage,
    ) -> FrameworkResult<Self> {
        let parent_id = parent_session_id.into();

        let metadata = SessionMetadata::new_subagent(
            session_id,
            agent_type,
            name,
            description,
            &parent_id,
            parent_tool_use_id,
        );

        // Persist the metadata
        storage.save_metadata(&metadata)?;

        // Update parent to track this child
        if let Ok(mut parent_meta) = storage.load_metadata(&parent_id) {
            parent_meta.add_child(&metadata.session_id);
            storage.save_metadata(&parent_meta)?;
        }

        Ok(Self {
            metadata,
            messages: Vec::new(),
            storage,
        })
    }

    /// Load an existing session from storage
    pub fn load(session_id: &str) -> FrameworkResult<Self> {
        let storage = SessionStorage::new();
        Self::load_with_storage(session_id, storage)
    }

    /// Load an existing session with custom storage
    pub fn load_with_storage(session_id: &str, storage: SessionStorage) -> FrameworkResult<Self> {
        let metadata = storage.load_metadata(session_id)?;
        let messages = storage.load_messages(session_id)?;

        Ok(Self {
            metadata,
            messages,
            storage,
        })
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.metadata.session_id
    }

    /// Get the agent type
    pub fn agent_type(&self) -> &str {
        &self.metadata.agent_type
    }

    /// Get the agent name
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Get the agent description
    pub fn description(&self) -> &str {
        &self.metadata.description
    }

    /// Check if this is a subagent session
    pub fn is_subagent(&self) -> bool {
        self.metadata.is_subagent()
    }

    /// Get the parent session ID (if this is a subagent)
    pub fn parent_session_id(&self) -> Option<&str> {
        self.metadata.parent_session_id.as_deref()
    }

    /// Get child session IDs
    pub fn child_session_ids(&self) -> &[String] {
        &self.metadata.child_session_ids
    }

    /// Add a message to the conversation history
    ///
    /// The message is immediately persisted to disk.
    pub fn add_message(&mut self, message: Message) -> FrameworkResult<()> {
        self.storage
            .append_message(&self.metadata.session_id, &message)?;
        self.messages.push(message);
        self.metadata.touch();
        self.storage.save_metadata(&self.metadata)?;
        Ok(())
    }

    /// Get the conversation history
    pub fn history(&self) -> &[Message] {
        &self.messages
    }

    /// Get a mutable reference to the conversation history
    ///
    /// Note: Changes made directly to this vector are not automatically persisted.
    /// Call `save()` after making changes.
    pub fn history_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Save the entire session (metadata and messages)
    ///
    /// This overwrites the existing history file.
    pub fn save(&mut self) -> FrameworkResult<()> {
        self.metadata.touch();
        self.storage.save_metadata(&self.metadata)?;
        self.storage
            .save_messages(&self.metadata.session_id, &self.messages)?;
        Ok(())
    }

    /// Reload the session from storage
    ///
    /// This discards any unsaved changes and reloads from disk.
    pub fn reload(&mut self) -> FrameworkResult<()> {
        self.metadata = self.storage.load_metadata(&self.metadata.session_id)?;
        self.messages = self.storage.load_messages(&self.metadata.session_id)?;
        Ok(())
    }

    /// Delete this session from storage
    ///
    /// Warning: This permanently deletes the session data.
    pub fn delete(self) -> FrameworkResult<()> {
        self.storage.delete_session(&self.metadata.session_id)
    }

    /// Get the storage backend
    pub fn storage(&self) -> &SessionStorage {
        &self.storage
    }

    /// Set the model for this session
    pub fn set_model(&mut self, model: impl Into<String>) {
        self.metadata.model = model.into();
        self.metadata.touch();
    }

    /// Get the model for this session
    pub fn model(&self) -> &str {
        &self.metadata.model
    }

    /// Set the provider for this session
    pub fn set_provider(&mut self, provider: impl Into<String>) {
        self.metadata.provider = provider.into();
        self.metadata.touch();
    }

    /// Get the provider for this session
    pub fn provider(&self) -> &str {
        &self.metadata.provider
    }

    /// Set custom metadata
    pub fn set_custom<T: Into<serde_json::Value>>(&mut self, key: impl Into<String>, value: T) {
        self.metadata.set_custom(key, value);
    }

    /// Get custom metadata
    pub fn get_custom(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get_custom(key)
    }

    /// List all sessions in storage
    pub fn list_all() -> FrameworkResult<Vec<String>> {
        SessionStorage::new().list_sessions()
    }

    /// List all sessions with custom storage
    pub fn list_all_with_storage(storage: &SessionStorage) -> FrameworkResult<Vec<String>> {
        storage.list_sessions()
    }

    /// Check if a session exists
    pub fn exists(session_id: &str) -> bool {
        SessionStorage::new().session_exists(session_id)
    }

    /// Check if a session exists with custom storage
    pub fn exists_with_storage(session_id: &str, storage: &SessionStorage) -> bool {
        storage.session_exists(session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (SessionStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = SessionStorage::with_dir(temp_dir.path());
        (storage, temp_dir)
    }

    #[test]
    fn test_new_session() {
        let (storage, _temp) = create_test_storage();

        let session =
            AgentSession::new_with_storage("test_session", "coder", "Test Coder", "A test agent", storage).unwrap();

        assert_eq!(session.session_id(), "test_session");
        assert_eq!(session.agent_type(), "coder");
        assert_eq!(session.name(), "Test Coder");
        assert_eq!(session.description(), "A test agent");
        assert!(!session.is_subagent());
        assert!(session.history().is_empty());
    }

    #[test]
    fn test_subagent_session() {
        let (storage, _temp) = create_test_storage();

        // Create parent first
        let _parent =
            AgentSession::new_with_storage("parent", "main", "Main Agent", "Parent agent", storage.clone())
                .unwrap();

        // Create subagent
        let subagent = AgentSession::new_subagent_with_storage(
            "sub_session",
            "researcher",
            "Research Agent",
            "Finds info",
            "parent",
            "tool_123",
            storage.clone(),
        )
        .unwrap();

        assert!(subagent.is_subagent());
        assert_eq!(subagent.parent_session_id(), Some("parent"));

        // Verify parent was updated
        let reloaded_parent = AgentSession::load_with_storage("parent", storage).unwrap();
        assert!(reloaded_parent
            .child_session_ids()
            .contains(&"sub_session".to_string()));
    }

    #[test]
    fn test_add_and_get_messages() {
        let (storage, _temp) = create_test_storage();

        let mut session =
            AgentSession::new_with_storage("msg_session", "coder", "Test", "Testing", storage.clone())
                .unwrap();

        // Add messages
        session.add_message(Message::user("Hello")).unwrap();
        session.add_message(Message::assistant("Hi there")).unwrap();

        assert_eq!(session.history().len(), 2);

        // Reload and verify persistence
        let reloaded = AgentSession::load_with_storage("msg_session", storage).unwrap();
        assert_eq!(reloaded.history().len(), 2);
    }

    #[test]
    fn test_save_and_reload() {
        let (storage, _temp) = create_test_storage();

        let mut session =
            AgentSession::new_with_storage("save_test", "coder", "Test", "Testing", storage.clone())
                .unwrap();

        // Add messages directly (bypassing append)
        session.messages.push(Message::user("Direct add"));
        session.save().unwrap();

        // Reload and verify
        let reloaded = AgentSession::load_with_storage("save_test", storage).unwrap();
        assert_eq!(reloaded.history().len(), 1);
    }

    #[test]
    fn test_delete_session() {
        let (storage, _temp) = create_test_storage();

        let session =
            AgentSession::new_with_storage("to_delete", "coder", "Test", "Testing", storage.clone())
                .unwrap();

        assert!(AgentSession::exists_with_storage("to_delete", &storage));

        session.delete().unwrap();

        assert!(!AgentSession::exists_with_storage("to_delete", &storage));
    }

    #[test]
    fn test_list_sessions() {
        let (storage, _temp) = create_test_storage();

        let _s1 =
            AgentSession::new_with_storage("session1", "coder", "S1", "D1", storage.clone()).unwrap();
        let _s2 =
            AgentSession::new_with_storage("session2", "researcher", "S2", "D2", storage.clone())
                .unwrap();

        let sessions = AgentSession::list_all_with_storage(&storage).unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&"session1".to_string()));
        assert!(sessions.contains(&"session2".to_string()));
    }

    #[test]
    fn test_custom_metadata() {
        let (storage, _temp) = create_test_storage();

        let mut session =
            AgentSession::new_with_storage("custom_test", "coder", "Test", "Testing", storage.clone())
                .unwrap();

        session.set_custom("key1", "value1");
        session.set_custom("count", serde_json::json!(42));
        session.save().unwrap();

        // Reload and verify
        let reloaded = AgentSession::load_with_storage("custom_test", storage).unwrap();
        assert_eq!(
            reloaded.get_custom("key1").and_then(|v| v.as_str()),
            Some("value1")
        );
        assert_eq!(
            reloaded.get_custom("count").and_then(|v| v.as_i64()),
            Some(42)
        );
    }

    #[test]
    fn test_model_and_provider() {
        let (storage, _temp) = create_test_storage();

        let mut session =
            AgentSession::new_with_storage("model_test", "coder", "Test", "Testing", storage).unwrap();

        session.set_model("claude-opus-4-5-20251101");
        session.set_provider("anthropic");

        assert_eq!(session.model(), "claude-opus-4-5-20251101");
        assert_eq!(session.provider(), "anthropic");
    }
}
