//! Operator HTTP surface: health, metrics, status, break-glass failover.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::RwLock;

use crate::health::PopHealth;
use crate::metrics::FailoverMetrics;
use crate::policy::{ManualOverride, PolicyEngine, ServingState};

/// Shared runtime snapshot for the status API.
#[derive(Debug, Clone)]
pub struct StatusSnapshot {
    pub dry_run: bool,
    pub ready: bool,
    pub pops: Vec<PopStatus>,
    pub hostnames: Vec<HostStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PopStatus {
    pub id: String,
    pub health: String,
    pub up: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostStatus {
    pub name: String,
    pub state: String,
    pub last_transition: Option<String>,
    pub last_applied: Option<String>,
    pub override_mode: String,
}

#[derive(Clone)]
pub struct ApiState {
    pub metrics: FailoverMetrics,
    pub status: Arc<RwLock<StatusSnapshot>>,
    pub policy: Arc<RwLock<PolicyEngine>>,
    /// SHA-256 of FAILOVER_API_TOKEN; None ⇒ mutating endpoints refuse (fail-closed
    /// unless listen is loopback-only — we still require a token when configured).
    pub api_token_sha256: Option<[u8; 32]>,
    pub reconcile_tx: tokio::sync::mpsc::Sender<()>,
}

impl std::fmt::Debug for ApiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiState")
            .field("token_configured", &self.api_token_sha256.is_some())
            .finish_non_exhaustive()
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/livez", get(livez))
        .route("/healthz", get(livez))
        .route("/readyz", get(readyz))
        .route("/ready", get(readyz))
        .route("/metrics", get(metrics))
        .route("/v1/status", get(status))
        .route("/v1/hostnames/:name/failover", post(force_failover))
        .route("/v1/hostnames/:name/failback", post(force_failback))
        .route("/v1/hostnames/:name/auto", post(clear_override))
        .route("/v1/hostnames/:name/reconcile", post(force_reconcile))
        .with_state(state)
}

async fn livez() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

async fn readyz(State(state): State<ApiState>) -> Response {
    let snap = state.status.read().await.clone();
    if snap.ready {
        (StatusCode::OK, Json(serde_json::json!({"status": "ready"}))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "not_ready"})),
        )
            .into_response()
    }
}

async fn metrics(State(state): State<ApiState>) -> Response {
    match state.metrics.render() {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            body,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn status(State(state): State<ApiState>) -> impl IntoResponse {
    let snap = state.status.read().await.clone();
    Json(serde_json::json!({
        "dry_run": snap.dry_run,
        "ready": snap.ready,
        "pops": snap.pops,
        "hostnames": snap.hostnames,
    }))
}

async fn force_failover(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    apply_override(
        &state,
        &name,
        &headers,
        ManualOverride::ForceSecondary,
        "secondary",
    )
    .await
}

async fn force_failback(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    apply_override(
        &state,
        &name,
        &headers,
        ManualOverride::ForcePrimary,
        "primary",
    )
    .await
}

/// Clear any manual override so health-driven policy decides again.
async fn clear_override(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    apply_override(&state, &name, &headers, ManualOverride::None, "none").await
}

async fn apply_override(
    state: &ApiState,
    name: &str,
    headers: &HeaderMap,
    mode: ManualOverride,
    label: &str,
) -> Response {
    if let Err(resp) = authorize(state, headers) {
        return resp;
    }
    let ok = state.policy.write().await.set_override(name, mode);
    if !ok {
        return unknown_hostname();
    }
    let _ = state.reconcile_tx.send(()).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({"ok": true, "override": label})),
    )
        .into_response()
}

async fn force_reconcile(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = authorize(&state, &headers) {
        return resp;
    }
    if !state.policy.read().await.manages(&name) {
        return unknown_hostname();
    }
    let _ = state.reconcile_tx.send(()).await;
    (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response()
}

fn unknown_hostname() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "unknown hostname"})),
    )
        .into_response()
}

#[allow(clippy::result_large_err)]
fn authorize(state: &ApiState, headers: &HeaderMap) -> Result<(), Response> {
    let Some(expected) = state.api_token_sha256 else {
        // No token configured: allow only if we treat mutating API as open —
        // fail-closed for mutations when token unset (matches nebula-api).
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "FAILOVER_API_TOKEN not configured",
                "code": "not_configured"
            })),
        )
            .into_response());
    };
    let presented = bearer_from(headers).unwrap_or("");
    let got = sha256_bytes(presented.as_bytes());
    if bool::from(expected.ct_eq(&got)) && !presented.is_empty() {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "unauthorized", "code": "unauthorized"})),
        )
            .into_response())
    }
}

fn bearer_from(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

pub fn token_sha256(token: &str) -> [u8; 32] {
    sha256_bytes(token.as_bytes())
}

pub fn pop_health_str(h: PopHealth) -> &'static str {
    match h {
        PopHealth::Up => "up",
        PopHealth::Down => "down",
        PopHealth::Unknown => "unknown",
    }
}

pub fn override_str(m: ManualOverride) -> &'static str {
    match m {
        ManualOverride::None => "none",
        ManualOverride::ForcePrimary => "force_primary",
        ManualOverride::ForceSecondary => "force_secondary",
    }
}

pub fn serving_str(s: ServingState) -> &'static str {
    s.as_str()
}
