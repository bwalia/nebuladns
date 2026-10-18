//! The NebulaDNS provider against a real record API: failover, failback, and a second
//! failover must each rewrite the record (regression: a per-owner Idempotency-Key made
//! every write after the first fail with 409).

use std::net::Ipv4Addr;

use nebula_api::auth::Auth;
use nebula_api::{control_plane_router, AppState};
use nebula_dns_failover::provider::{ApplyOutcome, NebulaDnsProvider, Provider};
use nebula_dns_failover::Config;
use nebula_metrics::Metrics;
use nebula_zone::{Zone, ZoneRegistry};

const TOKEN: &str = "provider-test-token";
const ZONE_TOML: &str = include_str!("../../../zones/example.com.toml");

async fn record_api() -> String {
    let zones = ZoneRegistry::new();
    zones.replace([Zone::from_toml(ZONE_TOML).unwrap()]);
    let state = AppState::new(Metrics::global())
        .with_zones(zones)
        .with_auth(Auth::from_parts(None, Some(TOKEN)).unwrap());
    state.set_ready(true);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, control_plane_router(state))
            .await
            .unwrap();
    });
    format!("http://{addr}")
}

fn provider(api_base: &str) -> NebulaDnsProvider {
    let cfg: Config = toml::from_str(&format!(
        r#"
dry_run = false
[provider]
type = "nebuladns"
api_base = "{api_base}"
api_token = "{TOKEN}"
zone_name = "example.com"
[pops.lon1]
public_ipv4 = "192.0.2.101"
health_urls = ["http://127.0.0.1:1/healthz"]
[pops.lon2]
public_ipv4 = "192.0.2.102"
health_urls = ["http://127.0.0.1:1/healthz"]
[[hostnames]]
name = "app.example.com"
mode = "active-passive"
primary = "lon1"
secondary = "lon2"
record_type = "A"
"#
    ))
    .unwrap();
    NebulaDnsProvider::new(&cfg).unwrap()
}

async fn values(p: &NebulaDnsProvider, name: &str, rtype: &str) -> Vec<String> {
    p.get_records(name)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.rtype.eq_ignore_ascii_case(rtype))
        .map(|r| r.content.trim_end_matches('.').to_string())
        .collect()
}

#[tokio::test]
async fn a_record_follows_failover_failback_and_second_failover() {
    let p = provider(&record_api().await);
    let lon1 = Ipv4Addr::new(192, 0, 2, 101);
    let lon2 = Ipv4Addr::new(192, 0, 2, 102);

    for (step, ip) in [
        ("primary", lon1),
        ("failover", lon2),
        ("failback", lon1),
        ("failover again", lon2),
    ] {
        let out = p
            .upsert_a("app.example.com", &[ip], 30, "", false, false)
            .await
            .unwrap_or_else(|e| panic!("{step}: {e}"));
        assert_eq!(out, ApplyOutcome::Applied, "{step}");
        assert_eq!(
            values(&p, "app.example.com", "A").await,
            [ip.to_string()],
            "{step}"
        );
    }

    // Already correct: no write.
    let out = p
        .upsert_a("app.example.com", &[lon2], 30, "", false, false)
        .await
        .unwrap();
    assert_eq!(out, ApplyOutcome::Noop);
}

#[tokio::test]
async fn cname_follows_failover_failback_and_second_failover() {
    let p = provider(&record_api().await);
    for (step, target) in [
        ("primary", "lon1.example.net"),
        ("failover", "lon2.example.net"),
        ("failback", "lon1.example.net"),
        ("failover again", "lon2.example.net"),
    ] {
        p.upsert_cname("edge.example.com", target, 30, "", false, false)
            .await
            .unwrap_or_else(|e| panic!("{step}: {e}"));
        assert_eq!(
            values(&p, "edge.example.com", "CNAME").await,
            [target],
            "{step}"
        );
    }
}
