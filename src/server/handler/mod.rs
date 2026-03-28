//! MCP request handler implementation
//!
//! Provides MCP protocol request handling logic, including tool listing, tool invocation, and resource lists.
//!
//! # Main Structs
//!
//! - `HandlerCore`: Shared core handling logic (internal use)
//! - `CratesDocsHandler`: Standard MCP handler
//! - `CratesDocsHandlerCore`: Core handler (provides more fine-grained control)
//!
//! # Design Pattern
//!
//! Uses composition pattern to eliminate code duplication:
//! - `HandlerCore` encapsulates all shared handling logic
//! - `CratesDocsHandler` and `CratesDocsHandlerCore` delegate to `HandlerCore`
//! - Supports config merging and optional metrics integration

mod config;
mod core;
mod core_handler;
mod standard;
mod types;

pub use config::HandlerConfig;
pub use core::HandlerCore;
pub use core_handler::CratesDocsHandlerCore;
pub use standard::CratesDocsHandler;
pub use types::ToolExecutionResult;
