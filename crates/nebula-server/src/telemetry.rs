//! Tracing / logging initialization.

use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::config::LoggingConfig;

/// Initialize the global tracing subscriber. Idempotent — subsequent calls are ignored.
pub fn init(cfg: &LoggingConfig) {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&cfg.filter))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry().with(filter);

    if cfg.json {
        let layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .with_target(true);
        let _ = registry.with(layer).try_init();
    } else {
        let layer = fmt::layer().with_target(true);
        let _ = registry.with(layer).try_init();
    }
}
