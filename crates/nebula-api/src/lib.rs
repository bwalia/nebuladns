//! Control-plane REST API for NebulaDNS.
//!
//! - `/livez` / `/readyz` / `/api/v1/version` — unauthenticated health
//! - `/api/v1/zones…` / `/api/v1/audit` — bearer-token zone access (fail closed)
//! - `/metrics` — Prometheus text exposition on a separate bind

#![forbid(unsafe_code)]

pub mod audit;
pub mod auth;
pub mod health;
pub mod metrics_endpoint;
pub mod records;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use nebula_metrics::Metrics;
use nebula_zone::ZoneRegistry;

use crate::audit::AuditLog;
use crate::auth::{Auth, AuthDecision};
use crate::records::{bearer_from, ApiError, IdempotencyCache, RecordPolicy, WriteLimiter};

/// Shared application state exposed to request handlers.
#[derive(Clone)]
pub struct AppState {
    pub metrics: Metrics,
    ready: Arc<AtomicBool>,
    pub zones: ZoneRegistry,
    pub auth: Auth,
    pub audit: AuditLog,
    pub policy: RecordPolicy,
    pub limiter: WriteLimiter,
    pub idem: IdempotencyCache,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("ready", &self.ready.load(Ordering::Relaxed))
            .field("auth", &self.auth)
            .field("zones", &self.zones)
            .finish_non_exhaustive()
    }
}

impl AppState {
    pub fn new(metrics: Metrics) -> Self {
        let policy = RecordPolicy::default();
        Self {
            metrics,
            ready: Arc::new(AtomicBool::new(false)),
            zones: ZoneRegistry::new(),
            auth: Auth::disabled(),
            audit: AuditLog::new(),
            limiter: WriteLimiter::new(policy.max_writes_per_sec),
            policy,
            idem: IdempotencyCache::default(),
        }
    }

    #[must_use]
    pub fn with_zones(mut self, zones: ZoneRegistry) -> Self {
        self.zones = zones;
        self
    }

    #[must_use]
    pub fn with_auth(mut self, auth: Auth) -> Self {
        self.auth = auth;
        self
    }

    #[must_use]
    pub fn with_policy(mut self, policy: RecordPolicy) -> Self {
        self.limiter = WriteLimiter::new(policy.max_writes_per_sec);
        self.policy = policy;
        self
    }

    /// Mark the process ready. Called once every startup subsystem has reported in.
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

/// Control-plane router. Health endpoints stay open; zone/record routes require a
/// configured bearer token (fail closed).
pub fn control_plane_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/v1/zones", get(records::list_zones))
        .route("/api/v1/zones/:zone", get(records::get_zone))
        .route(
            "/api/v1/zones/:zone/records",
            get(records::list_records)
                .put(records::put_record)
                .post(records::post_records)
                .delete(records::delete_record),
        )
        .route("/api/v1/audit", get(records::list_audit))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/api/v1/version", get(health::version))
        .merge(protected)
        .with_state(state)
}

async fn require_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    match state.auth.verify(bearer_from(request.headers())) {
        AuthDecision::Allow => next.run(request).await,
        AuthDecision::Deny => ApiError::Unauthorized.into_response(),
        AuthDecision::NotConfigured => ApiError::NotConfigured.into_response(),
    }
}

/// Metrics router: `/metrics` only, bound on a *separate* socket so a slow scraper
/// never backpressures DNS or control-plane traffic.
pub fn metrics_router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_endpoint::render))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use nebula_zone::Zone;
    use tower::ServiceExt;

    const ZONE: &str = r#"
origin = "example.com."
default_ttl = 300
[soa]
mname = "ns1.example.com."
rname = "hostmaster.example.com."
serial = 1
refresh = 10800
retry = 3600
expire = 604800
minimum = 300
[[records]]
name = "@"
type = "NS"
value = "ns1.example.com."
[[records]]
name = "www"
type = "A"
value = "192.0.2.10"
"#;

    fn authed_state() -> AppState {
        let zones = ZoneRegistry::new();
        zones.replace([Zone::from_toml(ZONE).unwrap()]);
        AppState::new(Metrics::global())
            .with_zones(zones)
            .with_auth(Auth::from_parts(None, Some("test-token")).unwrap())
    }

    async fn send(app: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    fn put_record(token: Option<&str>, body: &str) -> Request<Body> {
        let mut b = Request::builder()
            .method("PUT")
            .uri("/api/v1/zones/example.com/records")
            .header("content-type", "application/json");
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    #[tokio::test]
    async fn write_without_token_configured_is_forbidden() {
        let state = AppState::new(Metrics::global());
        let app = control_plane_router(state);
        let (status, json) = send(
            &app,
            put_record(
                Some("anything"),
                r#"{"name":"app","type":"CNAME","value":"west.example.net."}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "not_configured");
    }

    #[tokio::test]
    async fn write_with_wrong_token_is_unauthorized() {
        let app = control_plane_router(authed_state());
        let (status, _) = send(
            &app,
            put_record(
                Some("wrong"),
                r#"{"name":"app","type":"CNAME","value":"west.example.net."}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn livez_stays_open() {
        let app = control_plane_router(AppState::new(Metrics::global()));
        let req = Request::builder()
            .uri("/livez")
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(&app, req).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn cname_upsert_with_default_low_ttl() {
        let app = control_plane_router(authed_state());
        let (status, json) = send(
            &app,
            put_record(
                Some("test-token"),
                r#"{"name":"app","type":"CNAME","value":"west.example.net."}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(json["ok"], true);
        assert_eq!(json["records"][0]["ttl"], 5);
        assert_eq!(json["records"][0]["value"], "west.example.net.");
        assert_eq!(json["serial"], 2);
    }

    #[tokio::test]
    async fn apex_cname_rejected() {
        let app = control_plane_router(authed_state());
        let (status, json) = send(
            &app,
            put_record(
                Some("test-token"),
                r#"{"name":"@","type":"CNAME","value":"elsewhere.example.net."}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(json["error"].as_str().unwrap().contains("apex"));
    }

    #[tokio::test]
    async fn dry_run_does_not_bump_serial() {
        let app = control_plane_router(authed_state());
        let req = Request::builder()
            .method("PUT")
            .uri("/api/v1/zones/example.com/records?dry_run=true")
            .header("content-type", "application/json")
            .header("authorization", "Bearer test-token")
            .body(Body::from(
                r#"{"name":"app","type":"CNAME","value":"west.example.net."}"#,
            ))
            .unwrap();
        let (status, json) = send(&app, req).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["dry_run"], true);
        assert_eq!(json["serial"], 1);
    }

    #[tokio::test]
    async fn ttl_above_max_rejected() {
        let app = control_plane_router(authed_state());
        let (status, json) = send(
            &app,
            put_record(
                Some("test-token"),
                r#"{"name":"app","type":"CNAME","value":"west.example.net.","ttl":3600}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "validation");
    }

    #[tokio::test]
    async fn idempotent_replay_does_not_double_bump() {
        let app = control_plane_router(authed_state());
        let body = r#"{"name":"app","type":"CNAME","value":"west.example.net."}"#;
        let mk = || {
            Request::builder()
                .method("PUT")
                .uri("/api/v1/zones/example.com/records")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "failover-1")
                .body(Body::from(body.to_string()))
                .unwrap()
        };
        let (s1, j1) = send(&app, mk()).await;
        let (s2, j2) = send(&app, mk()).await;
        assert_eq!(s1, StatusCode::OK);
        assert_eq!(s2, StatusCode::OK);
        assert_eq!(j1["serial"], j2["serial"]);
        assert_eq!(j1["serial"], 2);
    }
}
