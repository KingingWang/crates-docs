//! Tool execution result types

use rust_mcp_sdk::schema::{CallToolError, CallToolResult, ResultFromServer};

/// Tool execution result (supports different return type conversions)
#[derive(Debug)]
pub struct ToolExecutionResult {
    /// Tool name
    pub tool_name: String,
    /// Execution duration
    pub duration: std::time::Duration,
    /// Whether successful
    pub success: bool,
    /// Original result (for converting to different types)
    pub result: std::result::Result<CallToolResult, CallToolError>,
}

impl ToolExecutionResult {
    /// Convert to `CallToolResult` (for `ServerHandler`)
    pub fn into_call_tool_result(self) -> std::result::Result<CallToolResult, CallToolError> {
        self.result
    }

    /// Convert to `ResultFromServer` (for `ServerHandlerCore`)
    pub fn into_result_from_server(self) -> ResultFromServer {
        self.result.unwrap_or_else(CallToolResult::from).into()
    }
}
