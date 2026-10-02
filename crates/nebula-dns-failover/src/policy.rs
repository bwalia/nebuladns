//! Desired-DNS policy: active-passive / active-active with fail-open.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::Instant;

use crate::config::{Config, FailoverMode, HostnameConfig, RecordType};
use crate::health::{HealthTracker, PopHealth};

/// High-level serving state for metrics / status API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServingState {
    Primary,
    Secondary,
    ActiveActive,
    DegradedBothDown,
    ManualPrimary,
    ManualSecondary,
}

impl ServingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::ActiveActive => "active_active",
            Self::DegradedBothDown => "degraded_both_down",
            Self::ManualPrimary => "manual_primary",
            Self::ManualSecondary => "manual_secondary",
        }
    }

    /// Numeric encoding for a Prometheus gauge.
    pub fn as_gauge(self) -> i64 {
        match self {
            Self::Primary => 1,
            Self::Secondary => 2,
            Self::ActiveActive => 3,
            Self::DegradedBothDown => 4,
            Self::ManualPrimary => 5,
            Self::ManualSecondary => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesiredTarget {
    /// One or more A records.
    A(Vec<Ipv4Addr>),
    /// Single CNAME target (no trailing-dot normalisation here — provider may add).
    Cname(String),
}

#[derive(Debug, Clone)]
pub struct DesireResult {
    pub target: DesiredTarget,
    pub state: ServingState,
    pub both_down: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualOverride {
    None,
    ForcePrimary,
    ForceSecondary,
}

#[derive(Debug, Clone)]
struct HostRuntime {
    last_applied: Option<DesiredTarget>,
    state: ServingState,
    override_mode: ManualOverride,
    /// When primary became eligible for failback (after consecutive_ok).
    primary_ok_since: Option<Instant>,
    last_transition: Option<String>,
}

impl Default for HostRuntime {
    fn default() -> Self {
        Self {
            last_applied: None,
            state: ServingState::Primary,
            override_mode: ManualOverride::None,
            primary_ok_since: None,
            last_transition: None,
        }
    }
}

/// Per-hostname policy + last-known-good tracking.
#[derive(Debug, Default)]
pub struct PolicyEngine {
    hosts: BTreeMap<String, HostRuntime>,
}

impl PolicyEngine {
    pub fn new(cfg: &Config) -> Self {
        let mut hosts = BTreeMap::new();
        for h in &cfg.hostnames {
            hosts.insert(h.name.clone(), HostRuntime::default());
        }
        Self { hosts }
    }

    pub fn set_override(&mut self, name: &str, mode: ManualOverride) -> bool {
        if let Some(h) = self.hosts.get_mut(name) {
            h.override_mode = mode;
            true
        } else {
            false
        }
    }

    pub fn clear_override(&mut self, name: &str) -> bool {
        self.set_override(name, ManualOverride::None)
    }

    pub fn mark_applied(&mut self, name: &str, target: DesiredTarget, state: ServingState) {
        let h = self.hosts.entry(name.to_string()).or_default();
        let prev = h.state;
        h.last_applied = Some(target);
        h.state = state;
        if prev != state {
            h.last_transition = Some(format!("{prev:?}->{state:?}"));
        }
    }

    pub fn state_of(&self, name: &str) -> ServingState {
        self.hosts
            .get(name)
            .map_or(ServingState::Primary, |h| h.state)
    }

    pub fn last_transition(&self, name: &str) -> Option<&str> {
        self.hosts
            .get(name)
            .and_then(|h| h.last_transition.as_deref())
    }

    pub fn last_applied(&self, name: &str) -> Option<&DesiredTarget> {
        self.hosts.get(name).and_then(|h| h.last_applied.as_ref())
    }

    pub fn override_of(&self, name: &str) -> ManualOverride {
        self.hosts
            .get(name)
            .map_or(ManualOverride::None, |h| h.override_mode)
    }

    pub fn desire(
        &mut self,
        cfg: &Config,
        host: &HostnameConfig,
        health: &HealthTracker,
    ) -> DesireResult {
        let runtime = self.hosts.entry(host.name.clone()).or_default();

        match runtime.override_mode {
            ManualOverride::ForcePrimary => {
                return DesireResult {
                    target: target_for_pops(cfg, host, &[host.primary.as_str()]),
                    state: ServingState::ManualPrimary,
                    both_down: false,
                    reason: "manual_force_primary".into(),
                };
            }
            ManualOverride::ForceSecondary => {
                return DesireResult {
                    target: target_for_pops(cfg, host, &[host.secondary.as_str()]),
                    state: ServingState::ManualSecondary,
                    both_down: false,
                    reason: "manual_force_secondary".into(),
                };
            }
            ManualOverride::None => {}
        }

        match host.mode {
            FailoverMode::ActivePassive => desire_active_passive(cfg, host, health, runtime),
            FailoverMode::ActiveActive => desire_active_active(cfg, host, health, runtime),
        }
    }
}

fn desire_active_passive(
    cfg: &Config,
    host: &HostnameConfig,
    health: &HealthTracker,
    runtime: &mut HostRuntime,
) -> DesireResult {
    let primary_up = health.is_up(&host.primary);
    let secondary_up = health.is_up(&host.secondary);

    // Unknown primary: do not failover yet (anti-flap / cold start).
    if health.health_of(&host.primary) == PopHealth::Unknown {
        return warming_hold(
            runtime,
            target_for_pops(cfg, host, &[host.primary.as_str()]),
            ServingState::Primary,
            "cold_start_prefer_primary",
        );
    }

    if primary_up {
        runtime.primary_ok_since.get_or_insert_with(Instant::now);
        if let Some(delay) = cfg.failback_delay {
            if let Some(since) = runtime.primary_ok_since {
                if since.elapsed() < delay
                    && matches!(
                        runtime.state,
                        ServingState::Secondary | ServingState::DegradedBothDown
                    )
                    && secondary_up
                {
                    return DesireResult {
                        target: target_for_pops(cfg, host, &[host.secondary.as_str()]),
                        state: ServingState::Secondary,
                        both_down: false,
                        reason: "failback_delay_soak".into(),
                    };
                }
            }
        }
        return DesireResult {
            target: target_for_pops(cfg, host, &[host.primary.as_str()]),
            state: ServingState::Primary,
            both_down: false,
            reason: "primary_up".into(),
        };
    }

    runtime.primary_ok_since = None;

    if secondary_up {
        return DesireResult {
            target: target_for_pops(cfg, host, &[host.secondary.as_str()]),
            state: ServingState::Secondary,
            both_down: false,
            reason: "primary_down_secondary_up".into(),
        };
    }

    // Both down (or secondary unknown while primary down): fail-open.
    if let Some(last) = runtime.last_applied.clone() {
        DesireResult {
            target: last,
            state: ServingState::DegradedBothDown,
            both_down: true,
            reason: "both_down_fail_open_last_good".into(),
        }
    } else {
        DesireResult {
            target: target_for_pops(cfg, host, &[host.primary.as_str()]),
            state: ServingState::DegradedBothDown,
            both_down: true,
            reason: "both_down_no_last_good_keep_primary".into(),
        }
    }
}

fn desire_active_active(
    cfg: &Config,
    host: &HostnameConfig,
    health: &HealthTracker,
    runtime: &HostRuntime,
) -> DesireResult {
    // Cold start: don't drop or alarm on POPs whose health isn't confirmed yet.
    if health.health_of(&host.primary) == PopHealth::Unknown
        || health.health_of(&host.secondary) == PopHealth::Unknown
    {
        return warming_hold(
            runtime,
            target_for_pops(cfg, host, &[host.primary.as_str(), host.secondary.as_str()]),
            ServingState::ActiveActive,
            "cold_start_seed_all",
        );
    }

    let mut ups = Vec::new();
    if health.is_up(&host.primary) {
        ups.push(host.primary.as_str());
    }
    if health.is_up(&host.secondary) {
        ups.push(host.secondary.as_str());
    }
    if ups.is_empty() {
        if let Some(last) = runtime.last_applied.clone() {
            return DesireResult {
                target: last,
                state: ServingState::DegradedBothDown,
                both_down: true,
                reason: "active_active_none_up_fail_open".into(),
            };
        }
        return DesireResult {
            target: target_for_pops(cfg, host, &[host.primary.as_str()]),
            state: ServingState::DegradedBothDown,
            both_down: true,
            reason: "active_active_none_up_seed_primary".into(),
        };
    }
    DesireResult {
        target: target_for_pops(cfg, host, &ups),
        state: ServingState::ActiveActive,
        both_down: false,
        reason: format!("active_active_up={}", ups.join("+")),
    }
}

/// Health is `Unknown` only until hysteresis first settles, so this is a
/// warm-up state, not an outage: hold the last answer (or seed one) without
/// changing serving state or flagging both-down.
fn warming_hold(
    runtime: &HostRuntime,
    seed: DesiredTarget,
    seed_state: ServingState,
    seed_reason: &str,
) -> DesireResult {
    match runtime.last_applied.clone() {
        Some(last) => DesireResult {
            target: last,
            state: runtime.state,
            both_down: false,
            reason: "cold_start_hold_last".into(),
        },
        None => DesireResult {
            target: seed,
            state: seed_state,
            both_down: false,
            reason: seed_reason.into(),
        },
    }
}

fn target_for_pops(cfg: &Config, host: &HostnameConfig, pops: &[&str]) -> DesiredTarget {
    match host.record_type {
        RecordType::A => {
            let ips: Vec<Ipv4Addr> = pops
                .iter()
                .filter_map(|id| cfg.pops.get(*id).map(|p| p.public_ipv4))
                .collect();
            DesiredTarget::A(ips)
        }
        RecordType::Cname => {
            // Active-active CNAME is not representable; use first up POP.
            let id = pops.first().copied().unwrap_or(host.primary.as_str());
            let target = host
                .cname_targets
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("{id}.invalid."));
            DesiredTarget::Cname(target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PopConfig, ProviderConfig, ProviderKind, RequireMode};
    use crate::health::HealthTracker;
    use std::time::Duration;

    fn test_cfg() -> Config {
        let mut pops = BTreeMap::new();
        pops.insert(
            "lon1".into(),
            PopConfig {
                display_name: None,
                public_ipv4: "195.20.255.201".parse().unwrap(),
                health_urls: vec!["http://x/healthz".into()],
                insecure_skip_verify: false,
                host_header: None,
            },
        );
        pops.insert(
            "lon2".into(),
            PopConfig {
                display_name: None,
                public_ipv4: "85.190.106.189".parse().unwrap(),
                health_urls: vec!["http://y/healthz".into()],
                insecure_skip_verify: false,
                host_header: None,
            },
        );
        Config {
            listen: "127.0.0.1:0".into(),
            dry_run: true,
            interval: Duration::from_secs(1),
            probe_timeout: Duration::from_secs(1),
            consecutive_fail: 3,
            consecutive_ok: 3,
            failback_delay: None,
            ttl: 30,
            require: RequireMode::All,
            provider: ProviderConfig {
                kind: ProviderKind::Nebuladns,
                api_base: Some("http://127.0.0.1:8080".into()),
                api_token: Some("t".into()),
                zone_name: "fictionally.org".into(),
                zone_id: None,
                cf_api_token: None,
            },
            pops,
            hostnames: vec![HostnameConfig {
                name: "abtesting.fictionally.org".into(),
                mode: FailoverMode::ActivePassive,
                primary: "lon1".into(),
                secondary: "lon2".into(),
                record_type: RecordType::A,
                cname_targets: BTreeMap::new(),
                proxied: false,
                enabled: true,
            }],
            api_token: None,
        }
    }

    fn mark_up(t: &mut HealthTracker, pop: &str) {
        for _ in 0..3 {
            t.force_sample(pop, true);
        }
    }

    fn mark_down(t: &mut HealthTracker, pop: &str) {
        for _ in 0..3 {
            t.force_sample(pop, false);
        }
    }

    #[test]
    fn primary_up_desires_primary() {
        let cfg = test_cfg();
        let mut health = HealthTracker::new(&cfg).unwrap();
        mark_up(&mut health, "lon1");
        mark_up(&mut health, "lon2");
        let mut eng = PolicyEngine::new(&cfg);
        let host = &cfg.hostnames[0];
        let d = eng.desire(&cfg, host, &health);
        assert_eq!(d.state, ServingState::Primary);
        assert_eq!(
            d.target,
            DesiredTarget::A(vec!["195.20.255.201".parse().unwrap()])
        );
        assert!(!d.both_down);
    }

    #[test]
    fn primary_down_secondary_up() {
        let cfg = test_cfg();
        let mut health = HealthTracker::new(&cfg).unwrap();
        mark_down(&mut health, "lon1");
        mark_up(&mut health, "lon2");
        let mut eng = PolicyEngine::new(&cfg);
        let host = &cfg.hostnames[0];
        let d = eng.desire(&cfg, host, &health);
        assert_eq!(d.state, ServingState::Secondary);
        assert_eq!(
            d.target,
            DesiredTarget::A(vec!["85.190.106.189".parse().unwrap()])
        );
    }

    #[test]
    fn both_down_keeps_last_good() {
        let cfg = test_cfg();
        let mut health = HealthTracker::new(&cfg).unwrap();
        mark_down(&mut health, "lon1");
        mark_down(&mut health, "lon2");
        let mut eng = PolicyEngine::new(&cfg);
        let host = &cfg.hostnames[0];
        eng.mark_applied(
            &host.name,
            DesiredTarget::A(vec!["195.20.255.201".parse().unwrap()]),
            ServingState::Primary,
        );
        let d = eng.desire(&cfg, host, &health);
        assert!(d.both_down);
        assert_eq!(d.state, ServingState::DegradedBothDown);
        assert_eq!(
            d.target,
            DesiredTarget::A(vec!["195.20.255.201".parse().unwrap()])
        );
    }

    #[test]
    fn warmup_never_reports_both_down() {
        let cfg = test_cfg();
        let health = HealthTracker::new(&cfg).unwrap();
        let mut eng = PolicyEngine::new(&cfg);
        let host = &cfg.hostnames[0];
        let primary = DesiredTarget::A(vec!["195.20.255.201".parse().unwrap()]);

        // Two cycles before hysteresis settles: seed, then hold.
        for reason in ["cold_start_prefer_primary", "cold_start_hold_last"] {
            let d = eng.desire(&cfg, host, &health);
            assert!(!d.both_down, "{reason}");
            assert_eq!(d.state, ServingState::Primary);
            assert_eq!(d.target, primary);
            assert_eq!(d.reason, reason);
            eng.mark_applied(&host.name, d.target, d.state);
        }
        assert_eq!(eng.last_transition(&host.name), None);
    }

    #[test]
    fn unknown_primary_does_not_failover() {
        let cfg = test_cfg();
        let mut health = HealthTracker::new(&cfg).unwrap();
        health.force_sample("lon1", false); // one failure: still Unknown
        mark_up(&mut health, "lon2");
        let mut eng = PolicyEngine::new(&cfg);
        let d = eng.desire(&cfg, &cfg.hostnames[0], &health);
        assert_eq!(d.state, ServingState::Primary);
        assert_eq!(
            d.target,
            DesiredTarget::A(vec!["195.20.255.201".parse().unwrap()])
        );
        assert!(!d.both_down);
    }

    #[test]
    fn active_active_warmup_keeps_both() {
        let mut cfg = test_cfg();
        cfg.hostnames[0].mode = FailoverMode::ActiveActive;
        let mut health = HealthTracker::new(&cfg).unwrap();
        mark_up(&mut health, "lon2"); // lon1 still Unknown
        let mut eng = PolicyEngine::new(&cfg);
        let d = eng.desire(&cfg, &cfg.hostnames[0], &health);
        assert!(!d.both_down);
        assert_eq!(d.state, ServingState::ActiveActive);
        assert_eq!(
            d.target,
            DesiredTarget::A(vec![
                "195.20.255.201".parse().unwrap(),
                "85.190.106.189".parse().unwrap(),
            ])
        );
    }
}
