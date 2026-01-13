# Design Decisions and Rationale

## Why JSON Lines for Conversation Storage?

**Decision:** Use JSON Lines (JSONL) format instead of a single JSON array or database.

**Rationale:**
1. **Append Efficiency**: Can append new messages without reading/parsing entire file
2. **Streaming**: Easy to stream conversations line-by-line
3. **Human Readable**: Can inspect conversations with text tools
4. **Recovery**: Corrupted lines don't invalidate entire conversation
5. **Simple**: No database dependencies or schema migrations

**Trade-offs:**
- ❌ Slightly less efficient for random access
- ✅ Perfect for append-heavy, sequential-read workloads (our use case)

---

## Permission Model: Per-Tool-Call vs Per-Session

**Decision:** Request permission for each individual tool call.

**Rationale:**
1. **Security**: User sees exactly what will execute
2. **Transparency**: No hidden actions
3. **Control**: User can deny dangerous operations
4. **Trust Building**: User learns to trust agent over time

**Future Enhancement:**
- Allow "trust this session" mode
- Remember permissions for specific commands
- Permission policies (always allow `ls`, always ask for `rm`)

---

## Trait-Based Architecture

**Decision:** Use traits for LLM providers, tools, and storage.

**Rationale:**
1. **Extensibility**: Easy to add new providers/tools without modifying core
2. **Testing**: Can mock providers and tools
3. **Plugin System**: Foundation for future plugin architecture
4. **Type Safety**: Rust's trait system ensures correctness

---

## Async from the Start

**Decision:** Use `tokio` and async/await throughout.

**Rationale:**
1. **LLM APIs**: All network I/O is async
2. **Streaming**: Natural fit for streaming responses
3. **Concurrency**: Can handle multiple operations (future multi-agent)
4. **Modern Rust**: Async is the standard for I/O-heavy apps

---

## Anthropic-First Approach

**Decision:** Implement Anthropic API first, then abstract.

**Rationale:**
1. **Known Target**: We understand Claude's behavior
2. **Tool Calling**: Anthropic has robust tool use support
3. **Iterate Fast**: Get something working, then generalize
4. **Provider Parity**: Can compare other providers against Claude

---

## Separation of Agent and CLI

**Decision:** Agent logic separate from UI/CLI concerns.

**Rationale:**
1. **Reusability**: Agent can be used in different contexts (API, web, CLI)
2. **Testing**: Can test agent logic without UI
3. **Future**: Enables web interface, API mode, etc.
4. **Clean Code**: Single responsibility principle

---

## Multi-Agent Design: Hierarchical vs Peer

**Decision:** Start with hierarchical (parent-child) model.

**Rationale:**
1. **Simpler**: Parent controls child, clear delegation
2. **Context**: Parent decides what context child needs
3. **Trust**: Parent is accountable for child's actions
4. **Evolution**: Can add peer-to-peer later if needed

**Future Consideration:**
- Peer agents for parallel task execution
- Agent pools with load balancing
- Specialized agent marketplace

---

## Configuration Philosophy

**Decision:** Sensible defaults, minimal required config.

**Rationale:**
1. **Quick Start**: Should work with just API key
2. **Progressive Disclosure**: Advanced features as needed
3. **Override Hierarchy**: CLI args > env vars > config file > defaults

---

## Error Handling Strategy

**Decision:** Use `anyhow` for application errors, `thiserror` for library errors.

**Rationale:**
1. **Application Layer**: `anyhow` for flexibility and context
2. **Library Layer**: `thiserror` for typed, matchable errors
3. **User-Friendly**: Convert errors to friendly messages in UI layer
4. **Debugging**: Preserve error chains for development

---

## Testing Strategy

**Decision:** Unit tests for logic, integration tests for flows, mocks for external APIs.

**Rationale:**
1. **Fast Tests**: Mock LLM providers for predictable tests
2. **Coverage**: Test tool execution, permission flows, conversation storage
3. **Examples**: Maintain working examples that double as smoke tests
4. **CI Ready**: Tests should run without API keys

---

## Why Not Use Existing Frameworks?

**Decision:** Build custom agent framework instead of using LangChain, etc.

**Rationale:**
1. **Learning**: Understand agent internals deeply
2. **Control**: Full control over behavior and architecture
3. **Simplicity**: No framework overhead, only what we need
4. **Rust Native**: Existing frameworks are Python-centric
5. **Performance**: Optimized for our use case

**Trade-offs:**
- ❌ More work upfront
- ✅ Exact fit for requirements
- ✅ No dependency bloat
- ✅ Deep understanding of system
