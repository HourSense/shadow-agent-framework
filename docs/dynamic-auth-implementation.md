# Dynamic Authentication Implementation

## Overview

Added support for dynamic authentication in `AnthropicProvider`, allowing API credentials to be refreshed before each request. This is essential for:
- JWT tokens that expire frequently (e.g., every 5 minutes)
- Proxy servers requiring per-request authentication
- Rotating API keys

## Files Created/Modified

### New Files

1. **`src/llm/auth.rs`**
   - `AuthConfig` - holds API key and optional base URL
   - `AuthProvider` trait - for implementing custom auth providers
   - `FnAuthProvider` - wrapper for async closure auth providers
   - `auth_provider()` - helper function to create providers from closures
   - `AuthSource` - internal enum for static vs dynamic auth

### Modified Files

1. **`src/llm/mod.rs`**
   - Added `pub mod auth;`
   - Exported `AuthConfig`, `AuthProvider`, `auth_provider`

2. **`src/llm/anthropic.rs`**
   - Changed `api_key: String` to `auth: AuthSource`
   - Added `ANTHROPIC_BASE_URL` env var support
   - Added `with_auth_provider()` constructor
   - Added `with_auth_provider_boxed()` for trait objects
   - Updated `send_request()` to call `auth.get_auth()` before each request
   - Updated `send_streaming_request()` similarly

## Usage

### Static Auth (existing behavior)

```rust
// From environment (ANTHROPIC_API_KEY, optional ANTHROPIC_BASE_URL)
let llm = AnthropicProvider::from_env()?;

// With explicit key
let llm = AnthropicProvider::new("sk-...")?;
```

### Dynamic Auth (new)

```rust
use shadow_agent_sdk::llm::{AnthropicProvider, AuthConfig};

// With async closure
let llm = AnthropicProvider::with_auth_provider(|| async {
    // Called before EACH API request
    let jwt = my_auth_service.get_fresh_token().await?;
    Ok(AuthConfig::with_base_url(jwt, "https://proxy.example.com/v1/messages"))
});

// With model configuration
let llm = AnthropicProvider::with_auth_provider(|| async {
    Ok(AuthConfig::new(get_token().await?))
})
.with_model("claude-sonnet-4-5-20250929")
.with_max_tokens(16000);
```

### Custom AuthProvider Implementation

```rust
use shadow_agent_sdk::llm::{AuthConfig, AuthProvider, AuthFuture};

struct MyAuthProvider {
    // Your auth state (token cache, refresh logic, etc.)
}

impl AuthProvider for MyAuthProvider {
    fn get_auth(&self) -> AuthFuture<'_> {
        Box::pin(async move {
            // Your refresh logic here
            Ok(AuthConfig::with_base_url(
                self.get_cached_or_refresh_token().await?,
                "https://my-proxy.com/v1/messages"
            ))
        })
    }
}

// Use it
let provider = Arc::new(MyAuthProvider::new());
let llm = AnthropicProvider::with_auth_provider_boxed(provider);
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `ANTHROPIC_API_KEY` | Yes (for `from_env`) | API key or token |
| `ANTHROPIC_BASE_URL` | No | Custom API endpoint |
| `ANTHROPIC_MODEL` | No | Model name (default: claude-sonnet-4-5-20250929) |
| `ANTHROPIC_MAX_TOKENS` | No | Max tokens (default: 32000) |

## Design Notes

1. **Auth is fetched before each API call** - This ensures fresh credentials even during long tool loops where a JWT might expire mid-turn.

2. **Caching is the provider's responsibility** - The `AuthProvider` implementation should handle caching logic internally (e.g., cache token until 30 seconds before expiry).

3. **Base URL per-request** - The base URL comes from `AuthConfig`, allowing different requests to potentially go to different endpoints (though typically it's constant).

4. **Backwards compatible** - Existing code using `from_env()` or `new()` continues to work unchanged.
