//! JSON-RPC 2.0 types for the MCP protocol.
//!
//! MCP (Model Context Protocol) frames every request, response, and notification as a
//! JSON-RPC 2.0 envelope, one per line on stdio. We model only the fields we actually
//! populate — a permissive `Value` tail captures anything extra so a newer client cannot
//! break us just by adding optional fields.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP protocol revision we advertise during `initialize`.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Incoming JSON-RPC message. A request has an `id`; a notification does not.
#[derive(Debug, Deserialize)]
pub struct Incoming {
    #[serde(default)]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// Outgoing JSON-RPC response (success or error).
#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
            data: None,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
            data: None,
        }
    }
}

/// Server-advertised capabilities. MCP allows every field to be optional; we only
/// advertise `tools` because that's the surface this server provides.
#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
pub struct ToolsCapability {
    /// Whether we emit `notifications/tools/list_changed`. We don't, so it's `false`.
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Serialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: &'static str,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: &'static str,
    pub version: &'static str,
}

/// A single MCP tool descriptor returned from `tools/list`.
#[derive(Debug, Serialize)]
pub struct ToolDescriptor {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema describing the tool's arguments. MCP clients use this to type-check
    /// the model's tool calls before they reach us.
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Content block returned by `tools/call`. We only ever emit `text`.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text { text: String },
}

#[derive(Debug, Serialize)]
pub struct ToolCallResult {
    pub content: Vec<Content>,
    /// Per MCP spec: `true` marks the content as a tool-level error the model should
    /// surface. Transport-level problems still come back as JSON-RPC errors.
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl ToolCallResult {
    pub fn text(body: String) -> Self {
        Self {
            content: vec![Content::Text { text: body }],
            is_error: false,
        }
    }

    pub fn error(body: String) -> Self {
        Self {
            content: vec![Content::Text { text: body }],
            is_error: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_ok_roundtrip() {
        let r = Response::ok(Value::from(1), serde_json::json!({"a": 1}));
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"jsonrpc\":\"2.0\""));
        assert!(s.contains("\"id\":1"));
        assert!(s.contains("\"result\""));
        assert!(!s.contains("\"error\""));
    }

    #[test]
    fn response_err_skips_result() {
        let r = Response::err(Value::from(7), RpcError::method_not_found("foo"));
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("\"result\""));
        assert!(s.contains("\"code\":-32601"));
        assert!(s.contains("foo"));
    }

    #[test]
    fn incoming_parses_notification() {
        let msg: Incoming =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .unwrap();
        assert_eq!(msg.method, "notifications/initialized");
        assert!(msg.id.is_none());
    }

    #[test]
    fn tool_call_result_error_flag_serializes() {
        let r = ToolCallResult::error("boom".into());
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"isError\":true"));
    }

    #[test]
    fn tool_call_result_ok_omits_error_flag() {
        let r = ToolCallResult::text("ok".into());
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("isError"));
    }
}
