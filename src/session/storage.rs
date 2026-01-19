//! Session storage helpers
//!
//! Handles reading and writing session data to disk.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::core::FrameworkResult;
use crate::core::error::FrameworkError;
use crate::llm::Message;

use super::metadata::SessionMetadata;

/// Default directory for session storage
const SESSIONS_DIR: &str = "sessions";

/// Session storage manager
#[derive(Debug, Clone)]
pub struct SessionStorage {
    base_dir: PathBuf,
}

impl SessionStorage {
    /// Create a new session storage with the default directory
    pub fn new() -> Self {
        Self {
            base_dir: PathBuf::from(SESSIONS_DIR),
        }
    }

    /// Create a new session storage with a custom directory
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: dir.into(),
        }
    }

    /// Get the directory path for a session
    pub fn session_dir(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(session_id)
    }

    /// Get the metadata file path for a session
    pub fn metadata_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("metadata.json")
    }

    /// Get the history file path for a session
    pub fn history_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("history.jsonl")
    }

    /// Create the session directory if it doesn't exist
    pub fn ensure_session_dir(&self, session_id: &str) -> FrameworkResult<PathBuf> {
        let dir = self.session_dir(session_id);
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    /// Save session metadata
    pub fn save_metadata(&self, metadata: &SessionMetadata) -> FrameworkResult<()> {
        self.ensure_session_dir(&metadata.session_id)?;
        let path = self.metadata_path(&metadata.session_id);

        let file = File::create(&path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, metadata)?;

        Ok(())
    }

    /// Load session metadata
    pub fn load_metadata(&self, session_id: &str) -> FrameworkResult<SessionMetadata> {
        let path = self.metadata_path(session_id);

        if !path.exists() {
            return Err(FrameworkError::SessionNotFound(session_id.to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let metadata: SessionMetadata = serde_json::from_reader(reader)?;

        Ok(metadata)
    }

    /// Append a message to the history file
    pub fn append_message(&self, session_id: &str, message: &Message) -> FrameworkResult<()> {
        self.ensure_session_dir(session_id)?;
        let path = self.history_path(session_id);

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        let json = serde_json::to_string(message)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    /// Load all messages from the history file
    pub fn load_messages(&self, session_id: &str) -> FrameworkResult<Vec<Message>> {
        let path = self.history_path(session_id);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let message: Message = serde_json::from_str(&line)?;
            messages.push(message);
        }

        Ok(messages)
    }

    /// Save all messages (overwrites existing history)
    pub fn save_messages(&self, session_id: &str, messages: &[Message]) -> FrameworkResult<()> {
        self.ensure_session_dir(session_id)?;
        let path = self.history_path(session_id);

        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        for message in messages {
            let json = serde_json::to_string(message)?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Check if a session exists
    pub fn session_exists(&self, session_id: &str) -> bool {
        self.metadata_path(session_id).exists()
    }

    /// List all session IDs
    pub fn list_sessions(&self) -> FrameworkResult<Vec<String>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        // Check if it has a metadata file
                        if self.metadata_path(name_str).exists() {
                            sessions.push(name_str.to_string());
                        }
                    }
                }
            }
        }

        Ok(sessions)
    }

    /// Delete a session
    pub fn delete_session(&self, session_id: &str) -> FrameworkResult<()> {
        let dir = self.session_dir(session_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Get the base directory
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

impl Default for SessionStorage {
    fn default() -> Self {
        Self::new()
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
    fn test_save_load_metadata() {
        let (storage, _temp) = create_test_storage();

        let meta = SessionMetadata::new("test_session", "coder", "Test", "Testing");
        storage.save_metadata(&meta).unwrap();

        let loaded = storage.load_metadata("test_session").unwrap();
        assert_eq!(loaded.session_id, "test_session");
        assert_eq!(loaded.agent_type, "coder");
    }

    #[test]
    fn test_append_load_messages() {
        let (storage, _temp) = create_test_storage();

        // Create session dir
        storage.ensure_session_dir("test_session").unwrap();

        // Append messages
        let msg1 = Message::user("Hello");
        let msg2 = Message::assistant("Hi there");

        storage.append_message("test_session", &msg1).unwrap();
        storage.append_message("test_session", &msg2).unwrap();

        // Load messages
        let messages = storage.load_messages("test_session").unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_session_exists() {
        let (storage, _temp) = create_test_storage();

        assert!(!storage.session_exists("nonexistent"));

        let meta = SessionMetadata::new("test_session", "coder", "Test", "Testing");
        storage.save_metadata(&meta).unwrap();

        assert!(storage.session_exists("test_session"));
    }

    #[test]
    fn test_list_sessions() {
        let (storage, _temp) = create_test_storage();

        // Create a few sessions
        storage.save_metadata(&SessionMetadata::new("session1", "coder", "S1", "D1")).unwrap();
        storage.save_metadata(&SessionMetadata::new("session2", "researcher", "S2", "D2")).unwrap();

        let sessions = storage.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&"session1".to_string()));
        assert!(sessions.contains(&"session2".to_string()));
    }

    #[test]
    fn test_delete_session() {
        let (storage, _temp) = create_test_storage();

        let meta = SessionMetadata::new("to_delete", "coder", "Test", "Testing");
        storage.save_metadata(&meta).unwrap();
        assert!(storage.session_exists("to_delete"));

        storage.delete_session("to_delete").unwrap();
        assert!(!storage.session_exists("to_delete"));
    }
}
