# Architecture Design

## High-Level Architecture

```
┌─────────────────────────────────────────────────┐
│              CLI Interface                      │
│  (User I/O, Permission Prompts, Display)       │
└────────────┬────────────────────────────────────┘
             │
┌────────────▼────────────────────────────────────┐
│           Agent Orchestrator                    │
│  (Main loop, Agent management)                  │
└────┬────────────────────────┬───────────────────┘
     │                        │
┌────▼─────────────┐   ┌──────▼──────────────────┐
│  Conversation    │   │   Tool Manager          │
│  Manager         │   │  (Registration,         │
│                  │   │   Execution,            │
│  - JSON Lines    │   │   Permission)           │
│  - History       │   │                         │
│  - Sessions      │   │  ┌─────────────────┐   │
└──────────────────┘   │  │ Bash Tool       │   │
                       │  └─────────────────┘   │
┌──────────────────┐   │  ┌─────────────────┐   │
│  LLM Provider    │   │  │ File Tool       │   │
│  Interface       │   │  └─────────────────┘   │
│                  │   │  ┌─────────────────┐   │
│  - Anthropic     │   │  │ (Future tools)  │   │
│  - OpenAI        │   │  └─────────────────┘   │
│  - Local         │   └─────────────────────────┘
└──────────────────┘
```

## Component Details

### CLI Interface
**Responsibilities:**
- Render messages with syntax highlighting
- Prompt user for input
- Display permission requests
- Show tool execution progress
- Handle keyboard interrupts

**Key Traits:**
- `Display`: Format and render messages
- `Input`: Capture user input
- `PermissionPrompt`: Get user approval for tool calls

### Agent Orchestrator
**Responsibilities:**
- Main conversation loop
- Manage agent lifecycle
- Route messages between components
- Handle multi-agent delegation

**Key Structs:**
- `Agent`: Main agent struct with capabilities
- `AgentConfig`: Configuration for agent behavior
- `AgentContext`: Runtime context (conversation, tools, etc.)

### Conversation Manager
**Responsibilities:**
- Persist messages to disk
- Load conversation history
- Manage multiple sessions
- Efficient appending

**File Format (JSON Lines):**
```json
{"role":"user","content":"Hello","timestamp":"2026-01-14T10:30:00Z"}
{"role":"assistant","content":"Hi!","timestamp":"2026-01-14T10:30:01Z"}
{"role":"user","content":"Run ls","timestamp":"2026-01-14T10:30:05Z"}
{"role":"assistant","tool_calls":[{"id":"1","type":"function","function":{"name":"bash","arguments":"{\"command\":\"ls\"}"}}],"timestamp":"2026-01-14T10:30:06Z"}
```

**Key Traits:**
- `ConversationStore`: Save/load conversations
- `Message`: Unified message format

### Tool Manager
**Responsibilities:**
- Register available tools
- Validate tool calls
- Request user permission
- Execute tools safely
- Return results

**Key Traits:**
- `Tool`: Common interface for all tools
- `ToolRegistry`: Manage available tools
- `PermissionManager`: Handle approval flow

**Tool Interface:**
```rust
trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn requires_permission(&self) -> bool;
    async fn execute(&self, params: serde_json::Value) -> Result<String>;
}
```

### LLM Provider Interface
**Responsibilities:**
- Abstract different LLM APIs
- Handle streaming responses
- Convert tool calling formats
- Manage API credentials

**Key Traits:**
- `LLMProvider`: Common interface
- `StreamHandler`: Handle streaming responses

**Provider Interface:**
```rust
trait LLMProvider {
    async fn send_message(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<Response>;

    fn supports_streaming(&self) -> bool;
    fn supports_tools(&self) -> bool;
}
```

## Data Flow

### Simple Request Flow
1. User enters message in CLI
2. Agent adds to conversation history
3. Agent sends conversation + tools to LLM
4. LLM responds (text or tool call)
5. If tool call:
   - CLI prompts user for permission
   - If approved, Tool Manager executes
   - Result added to conversation
   - Loop back to step 3
6. If text response:
   - Display to user
   - Wait for next input

### Multi-Agent Flow (Future)
1. Parent agent receives complex task
2. Decides to delegate to specialized agent
3. Creates child agent with:
   - Subset of tools
   - Relevant context
   - Specific instructions
4. Child agent executes task
5. Results returned to parent
6. Parent continues main task
