//! Prometheus metrics for the failover controller.

use std::sync::Arc;

use parking_lot::RwLock;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct PopLabels {
    pop: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct HostLabels {
    hostname: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct TransitionLabels {
    hostname: String,
    from: String,
    to: String,
    reason: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct ApplyLabels {
    hostname: String,
    result: String,
}

#[derive(Clone)]
pub struct FailoverMetrics {
    registry: Arc<RwLock<Registry>>,
    pop_up: Family<PopLabels, Gauge>,
    state: Family<HostLabels, Gauge>,
    transitions: Family<TransitionLabels, Counter>,
    dns_apply: Family<ApplyLabels, Counter>,
    /// Last probe latency in milliseconds (divide by 1000 for seconds).
    probe_latency_ms: Family<PopLabels, Gauge>,
    last_success: Gauge,
}

impl std::fmt::Debug for FailoverMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FailoverMetrics").finish_non_exhaustive()
    }
}

impl FailoverMetrics {
    pub fn new() -> Self {
        let mut registry = Registry::with_prefix("nebula_dns_failover");
        let pop_up = Family::<PopLabels, Gauge>::default();
        let state = Family::<HostLabels, Gauge>::default();
        let transitions = Family::<TransitionLabels, Counter>::default();
        let dns_apply = Family::<ApplyLabels, Counter>::default();
        let probe_latency_ms = Family::<PopLabels, Gauge>::default();
        let last_success = Gauge::default();

        registry.register(
            "pop_up",
            "Whether a POP is considered up (1) or down (0) after hysteresis.",
            pop_up.clone(),
        );
        registry.register(
            "state",
            "Serving state: 1=primary 2=secondary 3=active_active 4=degraded 5=manual_primary 6=manual_secondary.",
            state.clone(),
        );
        registry.register(
            "transitions_total",
            "Count of serving-state transitions.",
            transitions.clone(),
        );
        registry.register(
            "dns_apply_total",
            "DNS apply attempts by result (noop|planned|applied|error).",
            dns_apply.clone(),
        );
        registry.register(
            "probe_latency_milliseconds",
            "Last probe latency in milliseconds.",
            probe_latency_ms.clone(),
        );
        registry.register(
            "last_success_unixtime",
            "Unix time of last successful DNS apply.",
            last_success.clone(),
        );

        Self {
            registry: Arc::new(RwLock::new(registry)),
            pop_up,
            state,
            transitions,
            dns_apply,
            probe_latency_ms,
            last_success,
        }
    }

    pub fn set_pop_up(&self, pop: &str, up: u64) {
        self.pop_up
            .get_or_create(&PopLabels {
                pop: pop.to_string(),
            })
            .set(i64::try_from(up).unwrap_or(i64::MAX));
    }

    pub fn set_state(&self, hostname: &str, state: i64) {
        self.state
            .get_or_create(&HostLabels {
                hostname: hostname.to_string(),
            })
            .set(state);
    }

    pub fn inc_transition(&self, hostname: &str, from: &str, to: &str, reason: &str) {
        self.transitions
            .get_or_create(&TransitionLabels {
                hostname: hostname.to_string(),
                from: from.to_string(),
                to: to.to_string(),
                reason: reason.to_string(),
            })
            .inc();
    }

    pub fn inc_dns_apply(&self, hostname: &str, result: &str) {
        self.dns_apply
            .get_or_create(&ApplyLabels {
                hostname: hostname.to_string(),
                result: result.to_string(),
            })
            .inc();
    }

    pub fn observe_probe_latency(&self, pop: &str, seconds: f64) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = (seconds * 1000.0) as i64;
        self.probe_latency_ms
            .get_or_create(&PopLabels {
                pop: pop.to_string(),
            })
            .set(ms);
    }

    pub fn touch_last_success(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        self.last_success.set(now);
    }

    pub fn render(&self) -> Result<String, std::fmt::Error> {
        let guard = self.registry.read();
        let mut out = String::with_capacity(8 * 1024);
        encode(&mut out, &guard)?;
        Ok(out)
    }
}

impl Default for FailoverMetrics {
    fn default() -> Self {
        Self::new()
    }
}
