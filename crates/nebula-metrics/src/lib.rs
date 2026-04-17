//! Always-on Prometheus metrics for NebulaDNS.
//!
//! # Design contract (from `docs/architecture.md` §6)
//!
//! 1. Metrics are **always on in production**. No "enable metrics" flag.
//! 2. Zero hot-path allocation. Every counter/gauge is pre-registered at startup; hot paths
//!    do only relaxed atomic add/load.
//! 3. Label cardinality is bounded at compile time via enum-typed labels.
//! 4. Scrape cost must be < 10 ms for a 10k-zone deployment.
//! 5. Exposition runs on a separate worker from query serving.
//!
//! M0 scope: the registry wrapper, `build_info`, and the text-format exposition path. The
//! full catalogue of wire / transfer / DNSSEC / propagation metrics arrives in M2–M6 as
//! each subsystem lands.

#![forbid(unsafe_code)]

pub mod dns;

use std::sync::{Arc, LazyLock};

use parking_lot::RwLock;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

/// Build-info label set. All fields are `&'static str` — interned at compile/link time.
#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct BuildInfoLabels {
    pub version: String,
    pub commit: String,
    pub rustc: String,
    pub target: String,
}

/// A handle to the global metrics registry.
#[derive(Clone)]
pub struct Metrics {
    inner: Arc<RwLock<Registry>>,
    build_info: Family<BuildInfoLabels, Gauge>,
}

impl std::fmt::Debug for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metrics").finish_non_exhaustive()
    }
}

static GLOBAL: LazyLock<Metrics> = LazyLock::new(Metrics::new);

impl Metrics {
    fn new() -> Self {
        let mut registry = Registry::with_prefix("nebula");
        let build_info = Family::<BuildInfoLabels, Gauge>::default();
        registry.register(
            "build_info",
            "Build information (version, commit, rustc, target). Value is always 1.",
            build_info.clone(),
        );
        Self {
            inner: Arc::new(RwLock::new(registry)),
            build_info,
        }
    }

    /// Global singleton handle.
    pub fn global() -> Self {
        GLOBAL.clone()
    }

    /// Record this process's build-info. Called once at startup.
    pub fn set_build_info(&self, version: &str, commit: &str, rustc: &str, target: &str) {
        self.build_info
            .get_or_create(&BuildInfoLabels {
                version: version.to_string(),
                commit: commit.to_string(),
                rustc: rustc.to_string(),
                target: target.to_string(),
            })
            .set(1);
    }

    /// Give a subsystem a mutable registry reference to register its own metrics at
    /// startup. Never call on the hot path.
    pub fn with_registry_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Registry) -> R,
    {
        let mut guard = self.inner.write();
        f(&mut guard)
    }

    /// Render Prometheus text-format exposition.
    ///
    /// Reads hold a read lock on the registry; writes (registrations) take a write lock.
    /// Registrations happen only at startup, so there is no hot-path contention.
    pub fn render(&self) -> Result<String, std::fmt::Error> {
        let guard = self.inner.read();
        let mut out = String::with_capacity(64 * 1024);
        encode(&mut out, &guard)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_build_info() {
        let m = Metrics::new();
        m.set_build_info(
            "0.1.0-m0",
            "abcdef1",
            "rustc 1.80.0",
            "x86_64-unknown-linux-gnu",
        );
        let out = m.render().unwrap();
        assert!(out.contains("nebula_build_info"));
        assert!(out.contains("version=\"0.1.0-m0\""));
    }

    #[test]
    fn render_without_build_info_does_not_panic() {
        let m = Metrics::new();
        let out = m.render().unwrap();
        // Family with no child series still renders its HELP/TYPE preamble.
        assert!(out.contains("nebula_build_info"));
    }

    #[test]
    fn subsystems_can_register_via_with_registry_mut() {
        let m = Metrics::new();
        let counter = prometheus_client::metrics::counter::Counter::<u64>::default();
        m.with_registry_mut(|r| {
            r.register("test_counter", "A test counter.", counter.clone());
        });
        counter.inc();
        let out = m.render().unwrap();
        assert!(out.contains("nebula_test_counter"));
    }
}
