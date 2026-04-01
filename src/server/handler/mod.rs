//! MCP request handler implementation
//!
//! Provides MCP protocol request handling logic, including tool listing, tool invocation, and resource lists.
//!
//! # Main Structs
//!
//! - `CratesDocsHandler`: MCP handler implementing standard protocol interface
//! - `HandlerConfig`: Handler configuration class, supports merge operation
//!
//! # Design
//!
//! Single-layer architecture with all handling logic directly in `CratesDocsHandler`.

mod config;
mod standard;
mod types;

pub use config::HandlerConfig;
pub use standard::CratesDocsHandler;
pub use types::ToolExecutionResult;
