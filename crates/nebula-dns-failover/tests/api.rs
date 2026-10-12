//! Operator API integration tests: drive the real router against a real `PolicyEngine`
//! built from config, covering auth, overrides, override clear, and reconcile.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use nebula_dns_failover::api::{self, ApiState, HostStatus, StatusSnapshot};
use nebula_dns_failover::metrics::FailoverMetrics;
use nebula_dns_failover::policy::{ManualOverride, PolicyEngine};
use nebula_dns_failover::Config;
use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
use tower::ServiceExt;

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

struct Harness {
    state: ApiState,
    reconcile_rx: mpsc::Receiver<()>,
}

fn harness(token: Option<&str>) -> Harness {
    let cfg: Config = toml::from_str(CONFIG).expect("parse test config");
    cfg.validate().expect("valid test config");
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
        policy: Arc::new(RwLock::new(PolicyEngine::new(&cfg))),
        api_token_sha256: token.map(api::token_sha256),
        reconcile_tx,
    };
    Harness {
        state,
        reconcile_rx,
    }
}

async fn send(
    state: &ApiState,
    method: &str,
    path: &str,
    bearer: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let resp = api::router(state.clone())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn override_of(state: &ApiState) -> ManualOverride {
    state.policy.read().await.override_of(HOST)
}

#[tokio::test]
async fn status_is_readable_without_token() {
    let h = harness(Some(TOKEN));
    let (status, body) = send(&h.state, "GET", "/v1/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["dry_run"], true);
    assert_eq!(body["hostnames"][0]["name"], HOST);
}

#[tokio::test]
async fn mutations_refused_when_token_not_configured() {
    let mut h = harness(None);
    for action in ["failover", "failback", "auto", "reconcile"] {
        let path = format!("/v1/hostnames/{HOST}/{action}");
        let (status, body) = send(&h.state, "POST", &path, Some(TOKEN)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{action}");
        assert_eq!(body["code"], "not_configured", "{action}");
    }
    assert_eq!(override_of(&h.state).await, ManualOverride::None);
    assert!(
        h.reconcile_rx.try_recv().is_err(),
        "no reconcile on refusal"
    );
}

#[tokio::test]
async fn mutations_reject_missing_or_wrong_bearer() {
    let mut h = harness(Some(TOKEN));
    let path = format!("/v1/hostnames/{HOST}/failover");
    for bearer in [None, Some(""), Some("wrong-token")] {
        let (status, body) = send(&h.state, "POST", &path, bearer).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "bearer {bearer:?}");
        assert_eq!(body["code"], "unauthorized");
    }
    assert_eq!(override_of(&h.state).await, ManualOverride::None);
    assert!(
        h.reconcile_rx.try_recv().is_err(),
        "no reconcile on refusal"
    );
}

#[tokio::test]
async fn override_lifecycle_sets_policy_and_triggers_reconcile() {
    let mut h = harness(Some(TOKEN));
    let cases = [
        ("failover", "secondary", ManualOverride::ForceSecondary),
        ("failback", "primary", ManualOverride::ForcePrimary),
        ("auto", "none", ManualOverride::None),
    ];
    for (action, label, expected) in cases {
        let path = format!("/v1/hostnames/{HOST}/{action}");
        let (status, body) = send(&h.state, "POST", &path, Some(TOKEN)).await;
        assert_eq!(status, StatusCode::OK, "{action}");
        assert_eq!(body["ok"], true);
        assert_eq!(body["override"], label);
        assert_eq!(override_of(&h.state).await, expected, "{action}");
        h.reconcile_rx
            .try_recv()
            .unwrap_or_else(|_| panic!("{action} should queue a reconcile"));
    }
}

#[tokio::test]
async fn reconcile_queues_cycle_for_managed_hostname() {
    let mut h = harness(Some(TOKEN));
    let path = format!("/v1/hostnames/{HOST}/reconcile");
    let (status, body) = send(&h.state, "POST", &path, Some(TOKEN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    h.reconcile_rx.try_recv().expect("reconcile queued");
    assert_eq!(override_of(&h.state).await, ManualOverride::None);
}

#[tokio::test]
async fn unknown_hostname_is_404_for_every_action() {
    let mut h = harness(Some(TOKEN));
    for action in ["failover", "failback", "auto", "reconcile"] {
        let path = format!("/v1/hostnames/nope.fictionally.org/{action}");
        let (status, body) = send(&h.state, "POST", &path, Some(TOKEN)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{action}");
        assert_eq!(body["error"], "unknown hostname");
    }
    assert!(
        h.reconcile_rx.try_recv().is_err(),
        "no reconcile for unknown host"
    );
}
