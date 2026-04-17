//! `/metrics` handler. Renders the global Prometheus registry as text exposition.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::AppState;

const CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

/// Render Prometheus / OpenMetrics text exposition.
///
/// Returns 500 only if the underlying formatter fails, which is a programming bug — on the
/// production hot path this path is allocation-bounded and never panics.
pub async fn render(State(state): State<AppState>) -> Response {
    match state.metrics.render() {
        Ok(body) => ([(header::CONTENT_TYPE, CONTENT_TYPE)], body).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "metrics encoder failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "metrics render failed").into_response()
        }
    }
}
