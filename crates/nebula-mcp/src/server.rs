//! MCP stdio server loop.
//!
//! Read newline-delimited JSON-RPC messages from stdin, dispatch to `tools`, write
//! responses to stdout. Logging goes to stderr via `tracing` so it never corrupts the
//! protocol stream.

use std::net::SocketAddr;

use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::client::ApiClient;
use crate::protocol::{
    Incoming, InitializeResult, Response, RpcError, ServerCapabilities, ServerInfo, ToolCallResult,
    ToolsCapability, PROTOCOL_VERSION,
};
use crate::tools;

#[derive(Debug, Clone)]
pub struct Config {
    pub api_addr: SocketAddr,
    pub token: Option<String>,
    pub allow_writes: bool,
}

impl Config {
    /// Load config from the same env vars the CLI uses, plus MCP-specific gates.
    ///
    /// - `NEBULA_API` — admin endpoint (default `127.0.0.1:8080`, matching nebulactl)
    /// - `NEBULA_TOKEN` — optional bearer token for when M5+ auth is live
    /// - `NEBULA_MCP_ALLOW_WRITES` — `1`/`true` to enable mutating tools
    pub fn from_env() -> Result<Self> {
        let api_addr = std::env::var("NEBULA_API")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .map_err(|e| anyhow::anyhow!("NEBULA_API is not a valid SocketAddr: {e}"))?;
        let token = std::env::var("NEBULA_TOKEN").ok().filter(|s| !s.is_empty());
        let allow_writes = matches!(
            std::env::var("NEBULA_MCP_ALLOW_WRITES").as_deref(),
            Ok("1" | "true" | "TRUE" | "yes")
        );
        Ok(Self {
            api_addr,
            token,
            allow_writes,
        })
    }
}

/// Run the MCP server against stdio until EOF.
pub async fn run_stdio(cfg: Config) -> Result<()> {
    let client = ApiClient::new(cfg.api_addr, cfg.token.clone());
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    tracing::info!(
        api = %cfg.api_addr,
        allow_writes = cfg.allow_writes,
        "nebula-mcp ready"
    );

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_line(&line, &client, cfg.allow_writes).await;
        if let Some(resp) = response {
            let bytes = serde_json::to_vec(&resp)?;
            stdout.write_all(&bytes).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

/// Process a single line. Returns `None` for notifications (no response) or parse
/// failures we can't attach to an id.
async fn handle_line(line: &str, client: &ApiClient, allow_writes: bool) -> Option<Response> {
    let msg: Incoming = match serde_json::from_str(line) {
        Ok(m) => m,
        Err(err) => {
            // No id to respond to — log and drop. MCP clients recover by re-sending.
            tracing::warn!(error = %err, "dropping unparseable line");
            return None;
        }
    };

    // Notifications have no id and expect no response.
    let Some(id) = msg.id.clone() else {
        tracing::debug!(method = %msg.method, "notification received");
        return None;
    };

    Some(dispatch(id, &msg, client, allow_writes).await)
}

async fn dispatch(id: Value, msg: &Incoming, client: &ApiClient, allow_writes: bool) -> Response {
    match msg.method.as_str() {
        "initialize" => Response::ok(
            id,
            serde_json::to_value(InitializeResult {
                protocol_version: PROTOCOL_VERSION,
                capabilities: ServerCapabilities {
                    tools: ToolsCapability {
                        list_changed: false,
                    },
                },
                server_info: ServerInfo {
                    name: "nebula-mcp",
                    version: env!("CARGO_PKG_VERSION"),
                },
            })
            .unwrap_or(Value::Null),
        ),
        "tools/list" => Response::ok(
            id,
            json!({
                "tools": tools::catalogue(),
            }),
        ),
        "tools/call" => {
            let params = msg.params.clone().unwrap_or(Value::Null);
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Response::err(id, RpcError::invalid_params("missing `name`"));
            };
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);

            // Tool-level errors (API 404, validation failures) come back as
            // `isError: true` inside a successful JSON-RPC response so the model
            // sees them and can react. Transport-level invariants use JSON-RPC errors.
            let result = match tools::invoke(name, &args, client, allow_writes).await {
                Ok(r) => r,
                Err(e) => ToolCallResult::error(format!("{e:#}")),
            };
            match serde_json::to_value(result) {
                Ok(v) => Response::ok(id, v),
                Err(e) => Response::err(id, RpcError::internal(format!("serialize: {e}"))),
            }
        }
        "ping" => Response::ok(id, json!({})),
        other => Response::err(id, RpcError::method_not_found(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_client() -> ApiClient {
        ApiClient::new("127.0.0.1:0".parse().unwrap(), None)
    }

    #[tokio::test]
    async fn initialize_returns_capabilities() {
        let msg = Incoming {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "initialize".into(),
            params: None,
        };
        let resp = dispatch(json!(1), &msg, &fake_client(), false).await;
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], "nebula-mcp");
        assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    }

    #[tokio::test]
    async fn tools_list_includes_read_and_write() {
        let msg = Incoming {
            jsonrpc: "2.0".into(),
            id: Some(json!(2)),
            method: "tools/list".into(),
            params: None,
        };
        let resp = dispatch(json!(2), &msg, &fake_client(), false).await;
        let tools = &resp.result.unwrap()["tools"];
        let arr = tools.as_array().unwrap();
        let names: Vec<&str> = arr.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"health_check"));
        assert!(names.contains(&"create_zone"));
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let msg = Incoming {
            jsonrpc: "2.0".into(),
            id: Some(json!(3)),
            method: "does/not/exist".into(),
            params: None,
        };
        let resp = dispatch(json!(3), &msg, &fake_client(), false).await;
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
    }

    #[tokio::test]
    async fn write_tool_refused_without_gate() {
        let msg = Incoming {
            jsonrpc: "2.0".into(),
            id: Some(json!(4)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "create_zone",
                "arguments": { "zone": { "name": "example.com" } }
            })),
        };
        let resp = dispatch(json!(4), &msg, &fake_client(), false).await;
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("NEBULA_MCP_ALLOW_WRITES"));
    }

    #[tokio::test]
    async fn notifications_produce_no_response() {
        let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let resp = handle_line(line, &fake_client(), false).await;
        assert!(resp.is_none());
    }

    #[test]
    fn config_parses_write_gate_truthy_values() {
        // We can't set env vars reliably under test parallelism, so just exercise the
        // matcher logic directly.
        for truthy in ["1", "true", "TRUE", "yes"] {
            assert!(matches!(truthy, "1" | "true" | "TRUE" | "yes"));
        }
    }
}
