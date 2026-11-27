//! Serve checks: a real hostname is fetched at the POP's own IP (DNS pinned),
//! its status lands in metrics, and it never changes POP health.

use std::net::SocketAddr;

use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::Router;
use nebula_dns_failover::health::HealthTracker;
use nebula_dns_failover::metrics::FailoverMetrics;
use nebula_dns_failover::Config;
use tokio::net::TcpListener;

/// Serves `/healthz` 200 and, for `/`, 200 only when the Host header is
/// `good.invalid`; any other Host gets 502 (a vhost the POP can't serve).
async fn spawn_pop() -> SocketAddr {
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/",
            get(|headers: HeaderMap| async move {
                let host = headers
                    .get("host")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or_default();
                if host.starts_with("good.invalid") {
                    StatusCode::OK
                } else {
                    StatusCode::BAD_GATEWAY
                }
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
async fn serve_checks_pin_dns_to_pop_and_export_status() {
    let pop = spawn_pop().await;
    let port = pop.port();
    // `.invalid` never resolves publicly, so a pass proves the pin worked.
    let cfg: Config = toml::from_str(&format!(
        r#"
dry_run = true
probe_timeout = "2s"
consecutive_fail = 1
consecutive_ok = 1

[provider]
type = "nebuladns"
api_base = "http://127.0.0.1:1"
zone_name = "example.com"

[pops.lon2]
public_ipv4 = "127.0.0.1"
health_urls = ["http://127.0.0.1:{port}/healthz"]
serve_checks = [
  "http://good.invalid:{port}/",
  "http://bad.invalid:{port}/",
]

[[hostnames]]
name = "app.example.com"
primary = "lon2"
secondary = "lon2"
"#
    ))
    .unwrap();
    cfg.validate().unwrap();

    let metrics = FailoverMetrics::new();
    let mut health = HealthTracker::new(&cfg).unwrap();
    health.probe_all(&cfg, &metrics).await;

    // The failing vhost must not take the POP down.
    assert!(health.is_up("lon2"));

    let out = metrics.render().unwrap();
    for line in [
        format!(
            r#"nebula_dns_failover_serve_check_success{{pop="lon2",url="http://good.invalid:{port}/"}} 1"#
        ),
        format!(
            r#"nebula_dns_failover_serve_check_status_code{{pop="lon2",url="http://good.invalid:{port}/"}} 200"#
        ),
        format!(
            r#"nebula_dns_failover_serve_check_success{{pop="lon2",url="http://bad.invalid:{port}/"}} 0"#
        ),
        format!(
            r#"nebula_dns_failover_serve_check_status_code{{pop="lon2",url="http://bad.invalid:{port}/"}} 502"#
        ),
        format!(
            r#"nebula_dns_failover_probe_success{{pop="lon2",url="http://127.0.0.1:{port}/healthz"}} 1"#
        ),
        r#"nebula_dns_failover_consecutive_failures{pop="lon2"} 0"#.to_string(),
        r#"nebula_dns_failover_probes_total{pop="lon2",result="ok"} 1"#.to_string(),
    ] {
        assert!(out.contains(&line), "missing {line} in:\n{out}");
    }
}
