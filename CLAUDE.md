# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A high-performance Rust crate documentation query MCP server. Supports Stdio/HTTP/SSE transport protocols, TinyLFU/TTL memory caching (optional Redis), and provides tools for crate search, documentation lookup, and health checks.

## Build & Test Commands

```bash
# Build
cargo build
cargo build --release
cargo build --all-features

# Run clippy (both required before merge)
cargo clippy --all-targets -- -D warnings
cargo clippy --all-features --all-targets -- -D warnings

# Format check
cargo fmt -- --check

# Tests (multiple targets - specify correct one)
cargo test --test unit                   # Unit tests (primary)
cargo test --test unit_tests             # Legacy unit tests
cargo test --test integration_tests      # Integration tests
cargo test --test e2e                    # E2E tests

# Single test (must specify target)
cargo test --test unit test_name_here

# Feature-gated tests
cargo test --features cache-redis        # Redis cache tests

# All tests with all features
cargo test --all-features
```

## Feature Flags

| Feature | Purpose |
|---------|---------|
| `default` | server, stdio, macros, cache-memory, logging, api-key |
| `cache-redis` | Redis distributed cache support |
| `cache-memory` | Moka-based memory cache (in default) |
| `hyper-server` | HTTP server support |
| `sse` | Server-Sent Events transport |
| `streamable-http` | Streamable HTTP transport |
| `tls` | TLS/SSL support |
| `auth` | OAuth authentication |

## Architecture

```
src/
├── lib.rs           # Library entry, public API exports
├── main.rs          # Binary entry point
├── cache/           # Cache trait + MemoryCache/RedisCache implementations
├── cli/             # CLI commands (serve, test, config, health)
├── config/          # AppConfig, ServerConfig, CacheConfig, LoggingConfig
├── error/           # Error enum + Result alias
├── server/          # CratesDocsServer, handler, transport modes
├── tools/           # MCP tools (lookup_crate, search_crates, lookup_item, health_check)
│   └── docs/        # DocService, DocCache, HTML parsing
└── utils/           # HTTP client builder, rate limiter
```

### Key Data Flow

1. MCP client sends request via transport (stdio/http/sse)
2. CratesDocsHandler routes to ToolRegistry
3. Tool executes via DocService
4. DocService checks DocCache, if miss → fetches from docs.rs/crates.io
5. Result cached and returned

### Cache TTLs

- Crate docs: 3600s (1 hour)
- Item docs: 1800s (30 minutes)
- Search results: 300s (5 minutes)

## Code Style

- Edition 2021, pedantic clippy lints
- Use `crate::error::{Error, Result}` for library code
- Use `Arc` for shared async state (DocService, Cache)
- Import order: external crates (alphabetical), then local modules

## Test Isolation

- Do NOT use global `GLOBAL_HTTP_CLIENT` in tests
- Use `DocService::with_custom_client()` for test isolation
- Use `EnvVarGuard` or `#[serial_test]` for environment variable tests

## Adding New Tools

1. Create in `src/tools/{module}/`
2. Implement `Tool` trait (name, description, execute)
3. Register in `src/tools/mod.rs` via `ToolRegistry::register()`
4. Add tests in `tests/unit/`

## Important Files

- [AGENTS.md](AGENTS.md) - Detailed agent guidance (CI gates, patterns)
- [ARCHITECTURE.md](ARCHITECTURE.md) - Full system architecture
- [README.md](README.md) - User documentation