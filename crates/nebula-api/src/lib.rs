//! Control-plane REST API for NebulaDNS.
//!
//! M0 ships three operator-facing endpoints:
//!
//! - `/livez`   — liveness (process is alive)
//! - `/readyz`  — readiness (ready to serve)
//! - `/metrics` — Prometheus text exposition
//!
//! The full zone / deploy / secondary API surface lands in M5.

#![forbid(unsafe_code)]

pub mod health;
pub mod metrics_endpoint;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use axum::routing::get;
use axum::Router;
use nebula_metrics::Metrics;

/// Shared application state exposed to request handlers.
#[derive(Clone)]
pub struct AppState {
    pub metrics: Metrics,
    ready: Arc<AtomicBool>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("ready", &self.ready.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl AppState {
    pub fn new(metrics: Metrics) -> Self {
        Self {
            metrics,
            ready: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Mark the process ready. Called once every startup subsystem has reported in.
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

/// Control-plane router: `/livez`, `/readyz`, OpenAPI placeholder.
/// Runs on the admin bind (default `127.0.0.1:8080`).
pub fn control_plane_router(state: AppState) -> Router {
    Router::new()
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/api/v1/version", get(health::version))
        .with_state(state)
}

/// Metrics router: `/metrics` only, bound on a *separate* socket so a slow scraper
/// never backpressures DNS or control-plane traffic.
pub fn metrics_router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_endpoint::render))
        .with_state(state)
}
