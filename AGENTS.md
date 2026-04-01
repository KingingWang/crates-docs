# AGENTS.md

This file provides comprehensive guidance to agents when working with code in this repository.

## CI/CD Workflow & Code Gates

### Project Structure
- **Rust Edition**: 2021
- **Default Features**: server, stdio, macros, cache-memory, logging, api-key
- **Optional Features**: cache-redis, tls, auth, hyper-server, sse, streamable-http

### Build Commands

```bash
# Standard build
cargo build

# Build with all features
cargo build --all-features

# Build with specific feature
cargo build --features cache-redis

# Release build
cargo build --release
```

### Code Quality Gates (Must Pass Before Merge)

#### 1. Formatting Check
```bash
cargo fmt -- --check
```
- Checks if code is properly formatted
- Use `cargo fmt` to auto-fix formatting issues

#### 2. Clippy Linting
```bash
# Check without optional features
cargo clippy --all-targets -- -D warnings

# Check with all features (required - feature-gated code)
cargo clippy --all-features --all-targets -- -D warnings
```
**Critical**: Run both commands because Redis/auth code is feature-gated and checked separately.
- `-D warnings` treats all warnings as errors
- `--all-targets` includes lib, bins, tests, benches

#### 3. Security Audit
```bash
cargo install cargo-audit
cargo audit
```

### Testing Commands

#### Test Target Structure
Tests are split across THREE targets (not one suite):

```bash
# Unit tests (primary - new code goes here)
cargo test --test unit

# Legacy unit tests
cargo test --test unit_tests

# Integration tests
cargo test --test integration_tests

# E2E tests
cargo test --test e2e
```

#### Running Single Tests
**Important**: Use the correct target when running a single test:

```bash
# Correct: specify --test unit
cargo test --test unit test_oauth_config_github

# Correct: for tools_docs tests
cargo test --test unit test_lookup_crate_tool_execute_markdown

# Incorrect: will search all targets and may fail
cargo test test_oauth_config_github
```

#### Feature-Gated Testing

```bash
# Test with Redis cache (only when Redis is available)
cargo test --features cache-redis

# Test unit with Redis feature
cargo test --test unit --features cache-redis

# Test all features
cargo test --all-features
```

#### Multi-threaded Testing
Tests support concurrent execution. Default uses available cores:

```bash
# Use default threads
cargo test --test unit

# Force single thread (debugging)
cargo test --test unit -- --test-threads=1

# Use 4 threads
cargo test --test unit -- --test-threads=4
```

**Note**: Tests using `EnvVarGuard` or `serial_test` markers are handled for isolation.

### Documentation Gates

```bash
# Build documentation
cargo doc --no-deps --all-features

# Check documentation links
cargo doc --no-deps --all-features --document-private-items
```

### Coverage

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --all-features --out Xml
```

## Code Style Guidelines

### Top-Level Directives (src/lib.rs)
```rust
#![warn(missing_docs)]      // Require documentation on public items
#![warn(clippy::pedantic)]    // Enable pedantic lints
#![allow(clippy::module_name_repetitions)]  // Allow mod name repetition
#![allow(clippy::missing_errors_doc)]  // Allow missing error docs
#![allow(clippy::missing_panics_doc)] // Allow missing panic docs
```

### Import Style

```rust
// External crates (sorted alphabetically)
use reqwest::Client;
use serde_json::json;
use tokio::spawn;

// Local modules (alphabetical, grouped by module)
use crate::cache::{Cache, CacheConfig};
use crate::error::{Error, Result};
use crate::tools::docs::DocService;
```

### Naming Conventions

- **Types/Structs**: PascalCase (e.g., `DocService`, `CacheConfig`)
- **Functions/Methods**: snake_case (e.g., `fetch_html`, `with_config`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `VERSION`, `MAX_CONNECTIONS`)
- **Private fields**: snake_case (e.g., `client`, `cache`)
- **Modules**: snake_case (e.g., `cache`, `tools`, `utils`)

### Error Handling

**Library Code**: Use crate-specific Result/Error types:

```rust
// Define in src/error/mod.rs
#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(String),
}

// Use Result<T> alias
type Result<T> = std::result::Result<T, Error>;

// Convert foreign errors
map_err(|e| Error::http(e.to_string()))?;
```

**CLI Entry Points**: Return `Box<dyn std::error::Error>` for compatibility:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // CLI code
    Ok(())
}
```

### Public API Markers

```rust
// Mark functions that should use their return value
#[must_use]
pub fn create_config(&self) -> Config { }

// Mark test-only functions
#[cfg(test)]
pub fn test_helper() { }
```

### Async Patterns

```rust
// Async function with explicit lifetime
pub async fn fetch_data(&self, url: &str) -> Result<String> {
    let response = self.client.get(url).send().await?;
    response.text().await.map_err(Into::into)
}

// Use tokio::spawn for background tasks
tokio::spawn(async move {
    // Background work
});
```

### Config & Defaults

**Security**: Defaults are localhost-only:

```rust
// Default config restricts hosts/origins for security
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            allowed_hosts: vec!["127.0.0.1".to_string()],
            // ...
        }
    }
}
```

## Architecture Guidelines

### Server Initialization Order

```
1. Load config.toml (if present) or use defaults
2. Override config with CLI flags (explicit flags only, not env)
3. Create cache factory (memory or redis)
4. Create DocService with cache and config
5. Create tool registry via create_default_registry()
6. Initialize CratesDocsServer with registry and config
7. Run server (stdio, http, sse, or hybrid)
```

**Important**: Server does NOT use `AppConfig::from_env()` or `AppConfig::merge()` during startup.

### Cache Architecture

**Two cache formats exist** (do not unify):

1. **Raw Cache** (for search results):
   - Key: `search:{query}` (semantic)
   - Value: Serialized JSON from crates.io
   - TTL: 5 minutes

2. **Doc Cache** (for crate/item docs):
   - Key: `crate:{name}:{version}` or `item:{name}:{path}:{version}`
   - Value: Rendered Markdown/HTML
   - TTL: 1 hour (crate), 30 minutes (item)

Use `DocCache` for rendered docs, use raw cache interface for JSON.

### Test Isolation

**HTTP Client Testing**:
- Do NOT use global `GLOBAL_HTTP_CLIENT` in tests
- Use `DocService::with_custom_client()` for test isolation
- Each test creates its own HTTP client to avoid race conditions

**Environment Variables**:
- Use `EnvVarGuard` struct for safe cleanup
- Use `#[serial(group_name)]` from `serial_test` for tests that can't run in parallel
- Prefer `temp_env` crate over manual env var management

### Adding Tools

1. Implement tool struct in `src/tools/{module}/`
2. Implement `Tool` trait with:
   - `name()` - unique identifier
   - `description()` - user-facing description
   - `execute()` - async execution logic
3. Register in `src/tools/mod.rs` via `ToolRegistry::register()`
4. Add tests in `tests/unit/tools_docs_tests.rs`
5. Update `create_default_registry()` in `src/tools/mod.rs`

### Dependency Management

When adding a new dependency:
1. Check if it's the best/optimal choice for the use case
2. Use the latest stable version
3. Add minimal features needed
4. Run `cargo audit` to check for vulnerabilities
5. Update documentation if public API changes

## Common Patterns

### Optional Feature Handling

```rust
#[cfg(feature = "redis")]
impl Cache for RedisCache {
    fn get(&self, key: &str) -> Option<String> { }
}
```

### Arc/Async Pattern

```rust
// Arc for shared state
pub struct DocService {
    client: Arc<reqwest::Client>,
    cache: Arc<dyn Cache>,
}

// Clone Arcs for async tasks
let cache = self.cache.clone();
tokio::spawn(async move {
    cache.set("key", "value").await;
});
```

### Error Context

```rust
// Use tool_name parameter for better error messages
pub async fn fetch_html(&self, url: &str, tool_name: Option<&str>) -> Result<String> {
    let response = self.client.get(url).send().await.map_err(|e| {
        let prefix = tool_name.map_or(String::new(), |n| format!("[{n}] "));
        Error::http(format!("{prefix}HTTP failed: {e}"))
    })?;
    // ...
}
```

## Git Workflow

### Branching
- `main`/`master` - production
- `dev` - development/integration
- Feature branches - `feature/` prefix

### Commit Messages
Follow conventional commits:
- `feat:` new features
- `fix:` bug fixes
- `refactor:` code refactoring
- `test:` test additions/changes
- `docs:` documentation updates
- `chore:` maintenance tasks

### CI Triggers
- Push to `main`/`master` runs all checks
- Pull requests run all checks
- Tag push (`v*.*`) triggers release and publish

