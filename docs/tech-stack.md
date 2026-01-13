# Technology Stack

## Language
**Rust** - For performance, safety, and excellent CLI tooling

## Key Dependencies (Proposed)

### Core
- `tokio` - Async runtime for API calls and concurrent operations
- `serde` + `serde_json` - Serialization for messages and API communication
- `anyhow` / `thiserror` - Error handling

### HTTP & API
- `reqwest` - HTTP client for LLM provider APIs
- `async-stream` - Handle streaming responses

### CLI
- `clap` - Command-line argument parsing
- `crossterm` - Terminal manipulation (colors, cursor control)
- `dialoguer` - Interactive prompts and selections
- `indicatif` - Progress bars and spinners

### Storage
- `serde_json` - JSON Lines format
- `tokio::fs` - Async file operations

### String Processing
- `syntect` - Syntax highlighting for code blocks
- `pulldown-cmark` - Markdown parsing and rendering

### Configuration
- `config` - Application configuration management
- `directories` - Standard user directories (config, data, cache)

### Testing
- `mockito` - HTTP mocking for tests
- `tempfile` - Temporary files for tests

## Project Structure

```
singapore-project/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, CLI argument parsing
│   ├── lib.rs               # Library root
│   │
│   ├── agent/
│   │   ├── mod.rs           # Agent orchestrator
│   │   ├── config.rs        # Agent configuration
│   │   └── context.rs       # Runtime context
│   │
│   ├── conversation/
│   │   ├── mod.rs           # Conversation management
│   │   ├── message.rs       # Message types
│   │   ├── storage.rs       # JSON Lines persistence
│   │   └── history.rs       # History retrieval
│   │
│   ├── llm/
│   │   ├── mod.rs           # LLM provider trait
│   │   ├── anthropic.rs     # Anthropic implementation
│   │   ├── openai.rs        # OpenAI implementation
│   │   └── types.rs         # Common LLM types
│   │
│   ├── tools/
│   │   ├── mod.rs           # Tool trait and registry
│   │   ├── bash.rs          # Bash execution tool
│   │   ├── file.rs          # File operations
│   │   └── schema.rs        # Tool schema generation
│   │
│   ├── permissions/
│   │   ├── mod.rs           # Permission management
│   │   └── policy.rs        # Permission policies
│   │
│   └── ui/
│       ├── mod.rs           # CLI interface
│       ├── display.rs       # Message rendering
│       ├── input.rs         # User input handling
│       └── prompts.rs       # Permission prompts
│
├── docs/                    # Project documentation
├── tests/                   # Integration tests
└── examples/                # Example usage
```

## Configuration

Configuration will be stored in standard locations:
- **Config**: `~/.config/singapore-agent/config.toml`
- **Data**: `~/.local/share/singapore-agent/` (conversations)
- **Cache**: `~/.cache/singapore-agent/` (temp files)

Example `config.toml`:
```toml
[llm]
default_provider = "anthropic"

[llm.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-sonnet-4-5-20250929"
max_tokens = 4096

[llm.openai]
api_key_env = "OPENAI_API_KEY"
model = "gpt-4"

[permissions]
default_policy = "ask"  # ask, allow, deny

[ui]
syntax_highlighting = true
markdown_rendering = true
```
