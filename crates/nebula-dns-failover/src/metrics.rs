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
struct PopUrlLabels {
    pop: String,
    url: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct ProbeResultLabels {
    pop: String,
    result: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct TargetLabels {
    hostname: String,
    target: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
struct BuildLabels {
    version: String,
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
    probe_success: Family<PopUrlLabels, Gauge>,
    probes: Family<ProbeResultLabels, Counter>,
    consecutive_failures: Family<PopLabels, Gauge>,
    serve_check_success: Family<PopUrlLabels, Gauge>,
    serve_check_status: Family<PopUrlLabels, Gauge>,
    serve_check_latency_ms: Family<PopUrlLabels, Gauge>,
    serving_target: Family<TargetLabels, Gauge>,
    /// Last target exported per hostname, so a change drops the old series.
    current_targets: Arc<RwLock<std::collections::HashMap<String, String>>>,
    last_reconcile: Gauge,
}

impl std::fmt::Debug for FailoverMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FailoverMetrics").finish_non_exhaustive()
    }
}

impl FailoverMetrics {
    #[allow(clippy::too_many_lines)] // one register() per metric family
    pub fn new() -> Self {
        let mut registry = Registry::with_prefix("nebula_dns_failover");
        let pop_up = Family::<PopLabels, Gauge>::default();
        let state = Family::<HostLabels, Gauge>::default();
        let transitions = Family::<TransitionLabels, Counter>::default();
        let dns_apply = Family::<ApplyLabels, Counter>::default();
        let probe_latency_ms = Family::<PopLabels, Gauge>::default();
        let last_success = Gauge::default();
        let probe_success = Family::<PopUrlLabels, Gauge>::default();
        let probes = Family::<ProbeResultLabels, Counter>::default();
        let consecutive_failures = Family::<PopLabels, Gauge>::default();
        let serve_check_success = Family::<PopUrlLabels, Gauge>::default();
        let serve_check_status = Family::<PopUrlLabels, Gauge>::default();
        let serve_check_latency_ms = Family::<PopUrlLabels, Gauge>::default();
        let serving_target = Family::<TargetLabels, Gauge>::default();
        let last_reconcile = Gauge::default();
        let build_info = Family::<BuildLabels, Gauge>::default();

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
        // Counters get `_total` appended by prometheus_client; don't repeat it here.
        registry.register(
            "transitions",
            "Count of serving-state transitions.",
            transitions.clone(),
        );
        registry.register(
            "dns_apply",
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
            "Unix time of the last DNS write that changed a record (stays old while nothing flips; use last_reconcile_unixtime for liveness).",
            last_success.clone(),
        );
        registry.register(
            "last_reconcile_unixtime",
            "Unix time of the last reconcile pass in which every hostname applied without error (noop counts). Liveness signal.",
            last_reconcile.clone(),
        );
        registry.register(
            "probe_success",
            "Last health probe result per POP and health URL (1 ok, 0 failed).",
            probe_success.clone(),
        );
        registry.register(
            "probes",
            "Health probe rounds per POP by result (ok|fail), before hysteresis.",
            probes.clone(),
        );
        registry.register(
            "consecutive_failures",
            "Consecutive failed probe rounds for a POP (flips at consecutive_fail).",
            consecutive_failures.clone(),
        );
        registry.register(
            "serve_check_success",
            "Last serve check per POP and URL: the real hostname fetched at that POP's IP (1 = 2xx/3xx). Informational; never drives failover.",
            serve_check_success.clone(),
        );
        registry.register(
            "serve_check_status_code",
            "HTTP status of the last serve check (0 = transport error or timeout).",
            serve_check_status.clone(),
        );
        registry.register(
            "serve_check_latency_milliseconds",
            "Latency of the last serve check in milliseconds.",
            serve_check_latency_ms.clone(),
        );
        registry.register(
            "serving_target",
            "1 for the DNS target each hostname was last set to (A IPs comma-joined, or CNAME:<name>).",
            serving_target.clone(),
        );
        registry.register(
            "build_info",
            "Controller build; always 1.",
            build_info.clone(),
        );
        build_info
            .get_or_create(&BuildLabels {
                version: env!("CARGO_PKG_VERSION").to_string(),
            })
            .set(1);

        Self {
            registry: Arc::new(RwLock::new(registry)),
            pop_up,
            state,
            transitions,
            dns_apply,
            probe_latency_ms,
            last_success,
            probe_success,
            probes,
            consecutive_failures,
            serve_check_success,
            serve_check_status,
            serve_check_latency_ms,
            serving_target,
            current_targets: Arc::new(RwLock::new(std::collections::HashMap::new())),
            last_reconcile,
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
        self.last_success.set(now_unix());
    }

    pub fn set_probe_result(&self, pop: &str, url: &str, ok: bool) {
        self.probe_success
            .get_or_create(&PopUrlLabels {
                pop: pop.to_string(),
                url: url.to_string(),
            })
            .set(i64::from(ok));
    }

    /// One probe round for a POP (all its health URLs combined by `require`).
    pub fn record_probe_round(&self, pop: &str, ok: bool, consecutive_fail: u32) {
        self.probes
            .get_or_create(&ProbeResultLabels {
                pop: pop.to_string(),
                result: if ok { "ok" } else { "fail" }.to_string(),
            })
            .inc();
        self.consecutive_failures
            .get_or_create(&PopLabels {
                pop: pop.to_string(),
            })
            .set(i64::from(consecutive_fail));
    }

    /// `status` is the HTTP status, or 0 for a transport error / timeout.
    pub fn set_serve_check(&self, pop: &str, url: &str, status: u16, seconds: f64) {
        let labels = PopUrlLabels {
            pop: pop.to_string(),
            url: url.to_string(),
        };
        let ok = (200..400).contains(&status);
        self.serve_check_success
            .get_or_create(&labels)
            .set(i64::from(ok));
        self.serve_check_status
            .get_or_create(&labels)
            .set(i64::from(status));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = (seconds * 1000.0) as i64;
        self.serve_check_latency_ms.get_or_create(&labels).set(ms);
    }

    pub fn set_serving_target(&self, hostname: &str, target: &str) {
        let mut cur = self.current_targets.write();
        if let Some(prev) = cur.get(hostname) {
            if prev == target {
                return;
            }
            self.serving_target.remove(&TargetLabels {
                hostname: hostname.to_string(),
                target: prev.clone(),
            });
        }
        self.serving_target
            .get_or_create(&TargetLabels {
                hostname: hostname.to_string(),
                target: target.to_string(),
            })
            .set(1);
        cur.insert(hostname.to_string(), target.to_string());
    }

    pub fn touch_last_reconcile(&self) {
        self.last_reconcile.set(now_unix());
    }

    pub fn render(&self) -> Result<String, std::fmt::Error> {
        let guard = self.registry.read();
        let mut out = String::with_capacity(8 * 1024);
        encode(&mut out, &guard)?;
        Ok(out)
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Default for FailoverMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_names_match_dashboard_and_runbook() {
        let m = FailoverMetrics::new();
        m.set_pop_up("lon", 1);
        m.set_state("www.example.com", 1);
        m.inc_transition("www.example.com", "primary", "secondary", "pop_down");
        m.inc_dns_apply("www.example.com", "applied");
        m.observe_probe_latency("lon", 0.012);
        m.touch_last_success();
        m.touch_last_reconcile();
        m.set_probe_result("lon", "https://lon.example/healthz", true);
        m.record_probe_round("lon", true, 0);
        m.set_serve_check("lon", "https://www.example.com/", 502, 0.05);
        m.set_serving_target("www.example.com", "CNAME:lon1.example.");
        let out = m.render().unwrap();
        for name in [
            "nebula_dns_failover_pop_up{",
            "nebula_dns_failover_state{",
            "nebula_dns_failover_transitions_total{",
            "nebula_dns_failover_dns_apply_total{",
            "nebula_dns_failover_probe_latency_milliseconds{",
            "nebula_dns_failover_last_success_unixtime ",
            "nebula_dns_failover_last_reconcile_unixtime ",
            "nebula_dns_failover_probe_success{",
            "nebula_dns_failover_probes_total{",
            "nebula_dns_failover_consecutive_failures{",
            "nebula_dns_failover_serve_check_success{",
            "nebula_dns_failover_serve_check_status_code{",
            "nebula_dns_failover_serve_check_latency_milliseconds{",
            "nebula_dns_failover_serving_target{",
            "nebula_dns_failover_build_info{",
        ] {
            assert!(out.contains(name), "missing {name} in:\n{out}");
        }
        assert!(
            !out.contains("_total_total"),
            "counter name double-suffixed:\n{out}"
        );
    }

    #[test]
    fn serving_target_drops_previous_series() {
        let m = FailoverMetrics::new();
        m.set_serving_target("edge.example", "CNAME:lon1.example.");
        m.set_serving_target("edge.example", "CNAME:lon2.example.");
        let out = m.render().unwrap();
        assert!(out.contains(r#"target="CNAME:lon2.example.""#), "{out}");
        assert!(!out.contains(r#"target="CNAME:lon1.example.""#), "{out}");
    }

    #[test]
    fn serve_check_success_is_2xx_or_3xx() {
        let m = FailoverMetrics::new();
        m.set_serve_check("lon2", "https://a.example/", 307, 0.01);
        m.set_serve_check("lon2", "https://b.example/", 502, 0.01);
        m.set_serve_check("lon2", "https://c.example/", 0, 3.0);
        let out = m.render().unwrap();
        for (url, want) in [("a", 1), ("b", 0), ("c", 0)] {
            let line = format!(
                "nebula_dns_failover_serve_check_success{{pop=\"lon2\",url=\"https://{url}.example/\"}} {want}"
            );
            assert!(out.contains(&line), "missing {line} in:\n{out}");
        }
    }
}
