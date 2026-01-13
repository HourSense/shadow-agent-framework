# Coding Agent Project - Overview

## Vision
Build a lightweight but powerful coding agent in Rust that can interact with any LLM provider, execute tools with user permission, and support multi-agent workflows.

## Core Principles
1. **Provider Agnostic**: Work with any LLM provider (OpenAI, Anthropic, local models, etc.)
2. **Permission-Based**: User must explicitly approve each tool execution
3. **Conversation Persistence**: JSON Lines format for efficient append and retrieval
4. **Multi-Agent**: Support hierarchical agent delegation for specialized tasks
5. **Modular Architecture**: Clean separation of concerns

## Key Components

### 1. LLM Provider Abstraction
- Unified interface for different providers
- Handle API calls, streaming, token counting
- Support for tool/function calling

### 2. Conversation Manager
- Store messages in JSON Lines format
- Retrieve conversation history
- Support for multiple conversations/sessions
- Efficient appending without full file rewrites

### 3. CLI Interface
- Display messages with formatting
- Prompt user for permissions
- Handle user input
- Show tool execution status

### 4. Tool System
- Tool registration and discovery
- Permission checking before execution
- Standardized tool interface
- Initial tool: Bash execution

### 5. Multi-Agent Orchestration
- Parent agents can spawn specialized child agents
- Context and conversation passing
- Agent capability definitions

## Initial Scope (MVP)
Start with a simple single-agent system that can:
1. Connect to one LLM provider (e.g., Anthropic)
2. Maintain conversation in JSON Lines
3. Execute bash commands with user permission
4. Basic CLI interaction

Build from there incrementally.
