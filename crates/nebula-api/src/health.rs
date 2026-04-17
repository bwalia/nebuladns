//! Health and version endpoints.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::AppState;

#[derive(Debug, Serialize)]
pub struct HealthBody {
    pub status: &'static str,
}

/// Liveness: 200 OK as long as the process can respond. Never depends on external state.
pub async fn livez() -> impl IntoResponse {
    (StatusCode::OK, Json(HealthBody { status: "ok" }))
}

/// Readiness: 200 OK only once every startup subsystem has reported ready. Returns 503
/// during startup and shutdown so load balancers drain gracefully.
pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    if state.is_ready() {
        (StatusCode::OK, Json(HealthBody { status: "ready" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthBody { status: "starting" }),
        )
    }
}

#[derive(Debug, Serialize)]
pub struct VersionBody {
    pub version: &'static str,
    pub commit: &'static str,
    pub rustc: &'static str,
    pub target: &'static str,
}

/// Version info. Same facts as `nebula_build_info` in `/metrics` but served as JSON.
pub async fn version() -> impl IntoResponse {
    Json(VersionBody {
        version: env!("CARGO_PKG_VERSION"),
        commit: option_env!("NEBULA_GIT_COMMIT").unwrap_or("unknown"),
        rustc: option_env!("NEBULA_RUSTC_VERSION").unwrap_or("unknown"),
        target: option_env!("NEBULA_TARGET").unwrap_or("unknown"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn livez_is_ok() {
        let resp = livez().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_503_until_ready() {
        let state = AppState::new(nebula_metrics::Metrics::global());
        let resp = readyz(State(state.clone())).await.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

        state.set_ready(true);
        let resp = readyz(State(state)).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn version_returns_something() {
        let resp = version().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(body.get("version").is_some());
    }
}
