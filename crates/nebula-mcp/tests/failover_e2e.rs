//! End-to-end failover tools test: serve the real `nebula-dns-failover` operator API on
//! an ephemeral loopback port, spawn the `nebula-mcp` binary pointed at it, and drive
//! every `failover_*` tool over stdio — checking both the MCP responses and the
//! controller's policy state behind them.

use std::process::Stdio;
use std::sync::Arc;

use nebula_dns_failover::api::{self, ApiState, HostStatus, StatusSnapshot};
use nebula_dns_failover::metrics::FailoverMetrics;
use nebula_dns_failover::policy::{ManualOverride, PolicyEngine};
use nebula_dns_failover::Config;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, RwLock};

const TOKEN: &str = "test-failover-token";
const HOST: &str = "abtesting.fictionally.org";

const CONFIG: &str = r#"
listen = "127.0.0.1:0"
dry_run = true
interval = "10s"
probe_timeout = "3s"
consecutive_fail = 3
consecutive_ok = 3
ttl = 30

[provider]
type = "nebuladns"
api_base = "http://127.0.0.1:1"
zone_name = "fictionally.org"

[pops.lon1]
public_ipv4 = "195.20.255.201"
health_urls = ["http://127.0.0.1:1/healthz"]

[pops.lon2]
public_ipv4 = "85.190.106.189"
health_urls = ["http://127.0.0.1:1/healthz"]

[[hostnames]]
name = "abtesting.fictionally.org"
mode = "active-passive"
primary = "lon1"
secondary = "lon2"
record_type = "A"
"#;

struct Controller {
    addr: std::net::SocketAddr,
    policy: Arc<RwLock<PolicyEngine>>,
    reconcile_rx: mpsc::Receiver<()>,
}

impl Controller {
    async fn override_mode(&self) -> ManualOverride {
        self.policy.read().await.override_of(HOST)
    }

    /// Drain queued reconcile signals; returns how many there were.
    fn drain_reconciles(&mut self) -> usize {
        let mut n = 0;
        while self.reconcile_rx.try_recv().is_ok() {
            n += 1;
        }
        n
    }
}

async fn start_controller() -> Controller {
    let cfg: Config = toml::from_str(CONFIG).expect("parse test config");
    cfg.validate().expect("valid test config");
    let policy = Arc::new(RwLock::new(PolicyEngine::new(&cfg)));
    let (reconcile_tx, reconcile_rx) = mpsc::channel(16);
    let state = ApiState {
        metrics: FailoverMetrics::new(),
        status: Arc::new(RwLock::new(StatusSnapshot {
            dry_run: true,
            ready: true,
            pops: vec![],
            hostnames: vec![HostStatus {
                name: HOST.into(),
                state: "primary".into(),
                last_transition: None,
                last_applied: None,
                override_mode: "none".into(),
            }],
        })),
        policy: policy.clone(),
        api_token_sha256: Some(api::token_sha256(TOKEN)),
        reconcile_tx,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, api::router(state)).await.unwrap();
    });
    Controller {
        addr,
        policy,
        reconcile_rx,
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl Mcp {
    fn spawn(failover: std::net::SocketAddr, token: &str, allow_writes: bool) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nebula-mcp"))
            .env("NEBULA_API", "127.0.0.1:1") // unused by failover tools
            .env("NEBULA_FAILOVER_API", failover.to_string())
            .env("NEBULA_FAILOVER_TOKEN", token)
            .env(
                "NEBULA_MCP_ALLOW_WRITES",
                if allow_writes { "1" } else { "0" },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn nebula-mcp");
        let stdin = child.stdin.take().unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            stdin,
            lines,
            next_id: 1,
        }
    }

    async fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.stdin
            .write_all(format!("{req}\n").as_bytes())
            .await
            .unwrap();
        self.stdin.flush().await.unwrap();
        let line = self
            .lines
            .next_line()
            .await
            .unwrap()
            .expect("response line");
        let resp: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["id"], id);
        resp
    }

    /// Call a tool; returns `(is_error, text)`.
    async fn call(&mut self, tool: &str, args: Value) -> (bool, String) {
        let resp = self
            .rpc("tools/call", json!({ "name": tool, "arguments": args }))
            .await;
        let result = &resp["result"];
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        (result["isError"] == true, text)
    }

    async fn shutdown(mut self) {
        drop(self.stdin);
        let status = self.child.wait().await.unwrap();
        assert!(status.success(), "nebula-mcp exited non-zero: {status:?}");
    }
}

#[tokio::test]
async fn failover_tools_drive_controller_end_to_end() {
    let mut ctl = start_controller().await;
    let mut mcp = Mcp::spawn(ctl.addr, TOKEN, true);
    mcp.rpc("initialize", json!({ "protocolVersion": "2024-11-05" }))
        .await;
    let host = json!({ "hostname": HOST });

    // Read path.
    let (err, text) = mcp.call("failover_status", json!({})).await;
    assert!(!err, "failover_status failed: {text}");
    let status: Value = serde_json::from_str(&text).expect("status is JSON");
    assert_eq!(status["hostnames"][0]["name"], HOST);

    // Sticky override → swap → clear, each landing in the controller's policy.
    let steps = [
        (
            "failover_force_secondary",
            "secondary",
            ManualOverride::ForceSecondary,
        ),
        (
            "failover_force_primary",
            "primary",
            ManualOverride::ForcePrimary,
        ),
        ("failover_clear_override", "none", ManualOverride::None),
    ];
    for (tool, label, expected) in steps {
        let (err, text) = mcp.call(tool, host.clone()).await;
        assert!(!err, "{tool} failed: {text}");
        let body: Value = serde_json::from_str(&text).expect("JSON body");
        assert_eq!(body["override"], label, "{tool}");
        assert_eq!(ctl.override_mode().await, expected, "{tool}");
        assert_eq!(
            ctl.drain_reconciles(),
            1,
            "{tool} should queue one reconcile"
        );
    }

    let (err, text) = mcp.call("failover_reconcile", host.clone()).await;
    assert!(!err, "failover_reconcile failed: {text}");
    assert_eq!(ctl.drain_reconciles(), 1);

    // Controller errors surface as tool errors, not transport failures.
    let (err, text) = mcp
        .call(
            "failover_reconcile",
            json!({ "hostname": "nope.fictionally.org" }),
        )
        .await;
    assert!(err, "unknown hostname should be a tool error");
    assert!(text.contains("404"), "expected 404 in: {text}");
    assert_eq!(ctl.drain_reconciles(), 0);

    mcp.shutdown().await;
}

#[tokio::test]
async fn wrong_token_is_rejected_by_controller() {
    let mut ctl = start_controller().await;
    let mut mcp = Mcp::spawn(ctl.addr, "not-the-token", true);
    mcp.rpc("initialize", json!({ "protocolVersion": "2024-11-05" }))
        .await;

    let (err, text) = mcp
        .call("failover_force_secondary", json!({ "hostname": HOST }))
        .await;
    assert!(err, "bad token must not succeed");
    assert!(text.contains("401"), "expected 401 in: {text}");
    assert_eq!(ctl.override_mode().await, ManualOverride::None);
    assert_eq!(ctl.drain_reconciles(), 0);

    // Reads don't need the token.
    let (err, text) = mcp.call("failover_status", json!({})).await;
    assert!(!err, "failover_status failed: {text}");

    mcp.shutdown().await;
}

#[tokio::test]
async fn write_gate_blocks_before_reaching_controller() {
    let mut ctl = start_controller().await;
    let mut mcp = Mcp::spawn(ctl.addr, TOKEN, false);
    mcp.rpc("initialize", json!({ "protocolVersion": "2024-11-05" }))
        .await;

    let (err, text) = mcp
        .call("failover_force_secondary", json!({ "hostname": HOST }))
        .await;
    assert!(err);
    assert!(text.contains("NEBULA_MCP_ALLOW_WRITES"), "got: {text}");
    assert_eq!(ctl.override_mode().await, ManualOverride::None);
    assert_eq!(ctl.drain_reconciles(), 0);

    mcp.shutdown().await;
}
