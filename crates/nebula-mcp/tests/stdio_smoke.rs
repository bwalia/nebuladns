//! End-to-end stdio smoke test: spawn the binary, run `initialize` + `tools/list`,
//! confirm we get well-formed responses. No live nebula-api required — these RPCs are
//! purely server-local.

use std::process::Stdio;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

fn bin_path() -> std::path::PathBuf {
    // `CARGO_BIN_EXE_<name>` is set for the integration test by cargo.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_nebula-mcp"))
}

#[tokio::test]
async fn initialize_and_list_tools_over_stdio() {
    let mut child = Command::new(bin_path())
        .env("NEBULA_API", "127.0.0.1:1") // unused by these RPCs
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nebula-mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    // --- initialize ---
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2024-11-05" }
    });
    stdin
        .write_all(format!("{req}\n").as_bytes())
        .await
        .unwrap();
    stdin.flush().await.unwrap();

    let line = reader.next_line().await.unwrap().expect("response line");
    let resp: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["serverInfo"]["name"], "nebula-mcp");

    // --- tools/list ---
    let req = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    stdin
        .write_all(format!("{req}\n").as_bytes())
        .await
        .unwrap();
    stdin.flush().await.unwrap();

    let line = reader.next_line().await.unwrap().expect("response line");
    let resp: Value = serde_json::from_str(&line).unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty());
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"health_check"));
    assert!(names.contains(&"list_zones"));

    // Close stdin so the server exits cleanly.
    drop(stdin);
    let status = child.wait().await.unwrap();
    assert!(status.success(), "server exited non-zero: {status:?}");
}
