# Implementation Phases

## Phase 1: Foundation (MVP)
**Goal:** Basic working agent with single provider and bash tool

### Phase 1.1: Core Data Structures
- [ ] Define `Message` struct (user, assistant, system, tool)
- [ ] Define `ToolCall` and `ToolResult` structs
- [ ] Define `Conversation` struct
- [ ] Basic error types

### Phase 1.2: Conversation Storage
- [ ] Implement JSON Lines writer
- [ ] Implement JSON Lines reader
- [ ] Conversation append functionality
- [ ] Load conversation history
- [ ] Tests for persistence

### Phase 1.3: LLM Provider (Anthropic)
- [ ] Anthropic API client
- [ ] Convert messages to Anthropic format
- [ ] Handle tool calling format
- [ ] Parse responses
- [ ] Handle API errors
- [ ] Basic streaming support

### Phase 1.4: Tool System Basics
- [ ] Define `Tool` trait
- [ ] Implement `BashTool`
- [ ] Tool registry
- [ ] Tool schema generation for LLM

### Phase 1.5: Permission System
- [ ] Permission prompt in CLI
- [ ] Store permission decisions
- [ ] Permission policies (always allow, always deny, ask)

### Phase 1.6: Basic CLI
- [ ] Message display
- [ ] User input
- [ ] Permission prompts
- [ ] Basic formatting

### Phase 1.7: Agent Core Loop
- [ ] Initialize agent with config
- [ ] Main conversation loop
- [ ] Handle user messages
- [ ] Send to LLM
- [ ] Process tool calls
- [ ] Display responses

**Deliverable:** A working agent that can chat and execute bash commands with permission

---

## Phase 2: Multi-Provider Support
**Goal:** Support multiple LLM providers

- [ ] Define `LLMProvider` trait
- [ ] Refactor Anthropic to use trait
- [ ] Implement OpenAI provider
- [ ] Provider configuration
- [ ] Provider selection in CLI
- [ ] Handle provider-specific features

**Deliverable:** Switch between Anthropic, OpenAI, and potentially local models

---

## Phase 3: Enhanced CLI
**Goal:** Better user experience

- [ ] Syntax highlighting for code
- [ ] Markdown rendering
- [ ] Spinner for LLM requests
- [ ] Better error display
- [ ] Command history
- [ ] Tab completion
- [ ] Configuration commands (/config, /clear, etc.)

**Deliverable:** Professional CLI experience

---

## Phase 4: More Tools
**Goal:** Expand tool capabilities

- [ ] File read tool
- [ ] File write tool
- [ ] File edit tool (search/replace)
- [ ] Web search tool
- [ ] Code search (grep) tool
- [ ] Tool composition

**Deliverable:** Rich tool ecosystem

---

## Phase 5: Multi-Agent System
**Goal:** Support agent delegation

- [ ] Agent capability definitions
- [ ] Agent spawning mechanism
- [ ] Context passing between agents
- [ ] Agent communication protocol
- [ ] Sub-agent result integration
- [ ] Agent pools/specializations

**Deliverable:** Hierarchical multi-agent system

---

## Phase 6: Advanced Features
**Goal:** Production-ready features

- [ ] Token counting and budgets
- [ ] Cost tracking
- [ ] Conversation branching
- [ ] Agent memory/RAG
- [ ] Custom tool plugins
- [ ] Web interface (optional)
- [ ] API mode

---

## Current Focus: Phase 1.1 - Core Data Structures

This is where we should start. We'll build the fundamental types that everything else depends on.
