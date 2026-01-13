//! Debugger module for logging all API requests, responses, and tool calls
//!
//! This module creates a `debugger/` folder and stores detailed logs of:
//! - API requests sent to Anthropic
//! - API responses received
//! - Tool calls made and their results

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::llm::{MessageRequest, MessageResponse};

/// Global sequence counter for ordering events
static SEQUENCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Debugger for logging all agent activity
pub struct Debugger {
    /// Base directory for debug logs
    #[allow(dead_code)]
    base_dir: PathBuf,
    /// Session ID for this run
    #[allow(dead_code)]
    session_id: String,
    /// Session directory
    session_dir: PathBuf,
    /// Whether debugging is enabled
    enabled: bool,
}

/// A debug event entry
#[derive(Debug, Serialize)]
struct DebugEvent<T: Serialize> {
    sequence: u64,
    timestamp: DateTime<Utc>,
    event_type: String,
    data: T,
}

/// API request debug data
#[derive(Debug, Serialize)]
struct ApiRequestData {
    model: String,
    max_tokens: u32,
    message_count: usize,
    has_system_prompt: bool,
    tool_count: usize,
    has_thinking: bool,
    full_request: Value,
}

/// API response debug data
#[derive(Debug, Serialize)]
struct ApiResponseData {
    id: String,
    model: String,
    stop_reason: Option<String>,
    content_block_count: usize,
    input_tokens: u32,
    output_tokens: u32,
    full_response: Value,
}

/// Tool call debug data
#[derive(Debug, Serialize)]
struct ToolCallData {
    tool_use_id: String,
    tool_name: String,
    input: Value,
}

/// Tool result debug data
#[derive(Debug, Serialize)]
struct ToolResultData {
    tool_use_id: String,
    tool_name: String,
    is_error: bool,
    output: String,
    output_truncated: bool,
}

impl Debugger {
    /// Create a new debugger
    pub fn new() -> Result<Self> {
        let base_dir = std::env::current_dir()?.join("debugger");
        let session_id = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let session_dir = base_dir.join(&session_id);

        // Create the session directory
        fs::create_dir_all(&session_dir)?;

        tracing::info!("Debugger initialized: {:?}", session_dir);

        Ok(Self {
            base_dir,
            session_id,
            session_dir,
            enabled: true,
        })
    }

    /// Create a disabled debugger (no-op)
    pub fn disabled() -> Self {
        Self {
            base_dir: PathBuf::new(),
            session_id: String::new(),
            session_dir: PathBuf::new(),
            enabled: false,
        }
    }

    /// Check if debugging is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the next sequence number
    fn next_sequence(&self) -> u64 {
        SEQUENCE_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    /// Write a debug event to a file
    fn write_event<T: Serialize>(&self, event_type: &str, data: T) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let seq = self.next_sequence();
        let event = DebugEvent {
            sequence: seq,
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            data,
        };

        // Write to individual file
        let filename = format!("{:06}_{}.json", seq, event_type);
        let filepath = self.session_dir.join(&filename);
        let json = serde_json::to_string_pretty(&event)?;
        fs::write(&filepath, &json)?;

        // Also append to the combined log
        let log_path = self.session_dir.join("events.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        writeln!(file, "{}", serde_json::to_string(&event)?)?;

        tracing::debug!("Debug event written: {}", filename);

        Ok(())
    }

    /// Log an API request
    pub fn log_api_request(&self, request: &MessageRequest) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let data = ApiRequestData {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            message_count: request.messages.len(),
            has_system_prompt: request.system.is_some(),
            tool_count: request.tools.as_ref().map(|t| t.len()).unwrap_or(0),
            has_thinking: request.thinking.is_some(),
            full_request: serde_json::to_value(request)?,
        };

        self.write_event("api_request", data)
    }

    /// Log an API response
    pub fn log_api_response(&self, response: &MessageResponse) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let data = ApiResponseData {
            id: response.id.clone(),
            model: response.model.clone(),
            stop_reason: response.stop_reason.as_ref().map(|r| format!("{:?}", r)),
            content_block_count: response.content.len(),
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            full_response: serde_json::to_value(response)?,
        };

        self.write_event("api_response", data)
    }

    /// Log a tool call (before execution)
    pub fn log_tool_call(&self, tool_use_id: &str, tool_name: &str, input: &Value) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let data = ToolCallData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            input: input.clone(),
        };

        self.write_event("tool_call", data)
    }

    /// Log a tool result (after execution)
    pub fn log_tool_result(
        &self,
        tool_use_id: &str,
        tool_name: &str,
        output: &str,
        is_error: bool,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Truncate very long outputs for the debug log
        let (output_str, truncated) = if output.len() > 10000 {
            (format!("{}...[TRUNCATED]", &output[..10000]), true)
        } else {
            (output.to_string(), false)
        };

        let data = ToolResultData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            is_error,
            output: output_str,
            output_truncated: truncated,
        };

        self.write_event("tool_result", data)
    }

    /// Log a custom event with arbitrary data
    pub fn log_custom(&self, event_type: &str, data: Value) -> Result<()> {
        self.write_event(event_type, data)
    }

    /// Get the session directory path
    pub fn session_dir(&self) -> &PathBuf {
        &self.session_dir
    }
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self::disabled())
    }
}
