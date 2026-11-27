//! POP health probing with consecutive-failure hysteresis.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use reqwest::{redirect, Client};
use tracing::{debug, warn};

use crate::config::{Config, PopConfig, RequireMode};
use crate::metrics::FailoverMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopHealth {
    Up,
    Down,
    /// Not enough consecutive samples yet; treat as last known / unknown.
    Unknown,
}

#[derive(Debug, Clone)]
struct PopState {
    consecutive_ok: u32,
    consecutive_fail: u32,
    health: PopHealth,
    last_latency: Option<Duration>,
}

impl Default for PopState {
    fn default() -> Self {
        Self {
            consecutive_ok: 0,
            consecutive_fail: 0,
            health: PopHealth::Unknown,
            last_latency: None,
        }
    }
}

/// Tracks per-POP hysteresis and runs probes.
#[derive(Debug)]
pub struct HealthTracker {
    states: BTreeMap<String, PopState>,
    /// Per-POP clients whose DNS for every serve-check host is pinned to
    /// that POP's `public_ipv4`. Redirects are not followed (a 3xx is a
    /// pass, and following could leave the POP).
    serve_clients: BTreeMap<String, Client>,
    client: Client,
    insecure_client: Client,
    consecutive_fail: u32,
    consecutive_ok: u32,
    timeout: Duration,
    require: RequireMode,
}

impl HealthTracker {
    pub fn new(cfg: &Config) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(cfg.probe_timeout)
            .user_agent(concat!("nebula-dns-failover/", env!("CARGO_PKG_VERSION")))
            .build()?;
        let insecure_client = Client::builder()
            .timeout(cfg.probe_timeout)
            .danger_accept_invalid_certs(true)
            .user_agent(concat!("nebula-dns-failover/", env!("CARGO_PKG_VERSION")))
            .build()?;
        let mut states = BTreeMap::new();
        let mut serve_clients = BTreeMap::new();
        for (id, pop) in &cfg.pops {
            states.insert(id.clone(), PopState::default());
            if pop.serve_checks.is_empty() {
                continue;
            }
            let mut b = Client::builder()
                .timeout(cfg.probe_timeout)
                .redirect(redirect::Policy::none())
                .danger_accept_invalid_certs(pop.insecure_skip_verify)
                .user_agent(concat!(
                    "nebula-dns-failover/",
                    env!("CARGO_PKG_VERSION"),
                    " serve-check"
                ));
            for url in &pop.serve_checks {
                // Validated in Config::validate; skip quietly if a caller built
                // a Config by hand.
                let Ok(u) = reqwest::Url::parse(url) else {
                    continue;
                };
                let (Some(host), Some(port)) = (u.host_str(), u.port_or_known_default()) else {
                    continue;
                };
                b = b.resolve(host, SocketAddr::new(pop.public_ipv4.into(), port));
            }
            serve_clients.insert(id.clone(), b.build()?);
        }
        Ok(Self {
            states,
            serve_clients,
            client,
            insecure_client,
            consecutive_fail: cfg.consecutive_fail,
            consecutive_ok: cfg.consecutive_ok,
            timeout: cfg.probe_timeout,
            require: cfg.require,
        })
    }

    pub fn is_up(&self, pop: &str) -> bool {
        matches!(self.states.get(pop).map(|s| s.health), Some(PopHealth::Up))
    }

    pub fn health_of(&self, pop: &str) -> PopHealth {
        self.states
            .get(pop)
            .map_or(PopHealth::Unknown, |s| s.health)
    }

    pub fn snapshot(&self) -> BTreeMap<String, PopHealth> {
        self.states
            .iter()
            .map(|(k, v)| (k.clone(), v.health))
            .collect()
    }

    /// Probe every configured POP once and update hysteresis. Serve checks
    /// run concurrently with the health probes and only feed metrics.
    pub async fn probe_all(&mut self, cfg: &Config, metrics: &FailoverMetrics) {
        let this = &*self;
        let health = async {
            let mut out = Vec::with_capacity(cfg.pops.len());
            for (id, pop) in &cfg.pops {
                out.push((id.clone(), this.probe_pop(id, pop).await));
            }
            out
        };
        let (results, ()) = futures::join!(health, this.run_serve_checks(cfg, metrics));

        for (id, (per_url, last_latency)) in results {
            for (url, ok) in &per_url {
                metrics.set_probe_result(&id, url, *ok);
            }
            let ok = !per_url.is_empty()
                && match self.require {
                    RequireMode::All => per_url.iter().all(|(_, ok)| *ok),
                    RequireMode::Any => per_url.iter().any(|(_, ok)| *ok),
                };
            let (need_ok, need_fail) = (self.consecutive_ok, self.consecutive_fail);
            let state = self.states.entry(id.clone()).or_default();
            if last_latency.is_some() {
                state.last_latency = last_latency;
            }
            if ok {
                state.consecutive_ok = state.consecutive_ok.saturating_add(1);
                state.consecutive_fail = 0;
                if state.consecutive_ok >= need_ok {
                    state.health = PopHealth::Up;
                }
            } else {
                state.consecutive_fail = state.consecutive_fail.saturating_add(1);
                state.consecutive_ok = 0;
                if state.consecutive_fail >= need_fail {
                    state.health = PopHealth::Down;
                }
            }
            metrics.record_probe_round(&id, ok, state.consecutive_fail);
            let up = u64::from(state.health == PopHealth::Up);
            metrics.set_pop_up(&id, up);
            if let Some(lat) = state.last_latency {
                metrics.observe_probe_latency(&id, lat.as_secs_f64());
            }
            debug!(
                pop = %id,
                health = ?state.health,
                consecutive_ok = state.consecutive_ok,
                consecutive_fail = state.consecutive_fail,
                "probe result"
            );
        }
    }

    /// Returns each health URL's result and the latency of the last one.
    async fn probe_pop(
        &self,
        id: &str,
        pop: &PopConfig,
    ) -> (Vec<(String, bool)>, Option<Duration>) {
        if pop.health_urls.is_empty() {
            warn!(pop = %id, "no health_urls configured");
            return (Vec::new(), None);
        }
        let client = if pop.insecure_skip_verify {
            &self.insecure_client
        } else {
            &self.client
        };
        let mut results = Vec::with_capacity(pop.health_urls.len());
        let mut last_latency = None;
        for url in &pop.health_urls {
            let start = Instant::now();
            let mut req = client.get(url);
            if let Some(host) = &pop.host_header {
                req = req.header(reqwest::header::HOST, host);
            }
            let ok = match tokio::time::timeout(self.timeout, req.send()).await {
                Ok(Ok(resp)) => resp.status().is_success(),
                Ok(Err(err)) => {
                    debug!(pop = %id, %url, error = %err, "probe transport error");
                    false
                }
                Err(_) => {
                    debug!(pop = %id, %url, "probe timeout");
                    false
                }
            };
            last_latency = Some(start.elapsed());
            results.push((url.clone(), ok));
        }
        (results, last_latency)
    }

    async fn run_serve_checks(&self, cfg: &Config, metrics: &FailoverMetrics) {
        let checks = cfg.pops.iter().flat_map(|(id, pop)| {
            let client = self.serve_clients.get(id);
            pop.serve_checks
                .iter()
                .filter_map(move |url| client.map(|c| (id, url, c)))
        });
        let futs = checks.map(|(id, url, client)| async move {
            let start = Instant::now();
            let status = match tokio::time::timeout(self.timeout, client.get(url).send()).await {
                Ok(Ok(resp)) => resp.status().as_u16(),
                Ok(Err(err)) => {
                    debug!(pop = %id, %url, error = %err, "serve check transport error");
                    0
                }
                Err(_) => {
                    debug!(pop = %id, %url, "serve check timeout");
                    0
                }
            };
            metrics.set_serve_check(id, url, status, start.elapsed().as_secs_f64());
        });
        futures::future::join_all(futs).await;
    }

    /// Test helper: force a POP health after enough consecutive samples.
    #[cfg(test)]
    pub fn force_sample(&mut self, pop: &str, ok: bool) {
        let state = self.states.entry(pop.to_string()).or_default();
        if ok {
            state.consecutive_ok = state.consecutive_ok.saturating_add(1);
            state.consecutive_fail = 0;
            if state.consecutive_ok >= self.consecutive_ok {
                state.health = PopHealth::Up;
            }
        } else {
            state.consecutive_fail = state.consecutive_fail.saturating_add(1);
            state.consecutive_ok = 0;
            if state.consecutive_fail >= self.consecutive_fail {
                state.health = PopHealth::Down;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HostnameConfig, ProviderConfig, ProviderKind};
    use std::net::Ipv4Addr;

    fn cfg() -> Config {
        let mut pops = BTreeMap::new();
        pops.insert(
            "lon1".into(),
            PopConfig {
                display_name: None,
                public_ipv4: "195.20.255.201".parse().unwrap(),
                health_urls: vec!["http://127.0.0.1:9/healthz".into()],
                insecure_skip_verify: false,
                host_header: None,
                serve_checks: Vec::new(),
            },
        );
        pops.insert(
            "lon2".into(),
            PopConfig {
                display_name: None,
                public_ipv4: Ipv4Addr::new(85, 190, 106, 189),
                health_urls: vec!["http://127.0.0.1:9/healthz".into()],
                insecure_skip_verify: false,
                host_header: None,
                serve_checks: Vec::new(),
            },
        );
        Config {
            listen: "127.0.0.1:0".into(),
            dry_run: true,
            interval: Duration::from_secs(1),
            probe_timeout: Duration::from_millis(50),
            consecutive_fail: 3,
            consecutive_ok: 3,
            failback_delay: None,
            ttl: 30,
            require: RequireMode::All,
            provider: ProviderConfig {
                kind: ProviderKind::Nebuladns,
                api_base: Some("http://127.0.0.1:8080".into()),
                api_token: Some("t".into()),
                zone_name: "example.com".into(),
                zone_id: None,
                cf_api_token: None,
            },
            pops,
            hostnames: vec![HostnameConfig {
                name: "app.example.com".into(),
                zone_name: None,
                mode: crate::config::FailoverMode::ActivePassive,
                primary: "lon1".into(),
                secondary: "lon2".into(),
                record_type: crate::config::RecordType::A,
                cname_targets: BTreeMap::new(),
                proxied: false,
                enabled: true,
            }],
            api_token: None,
        }
    }

    #[test]
    fn hysteresis_requires_n_failures() {
        let c = cfg();
        let mut t = HealthTracker::new(&c).unwrap();
        assert_eq!(t.health_of("lon1"), PopHealth::Unknown);
        t.force_sample("lon1", false);
        t.force_sample("lon1", false);
        assert_eq!(t.health_of("lon1"), PopHealth::Unknown);
        t.force_sample("lon1", false);
        assert_eq!(t.health_of("lon1"), PopHealth::Down);
    }

    #[test]
    fn hysteresis_requires_n_successes() {
        let c = cfg();
        let mut t = HealthTracker::new(&c).unwrap();
        for _ in 0..3 {
            t.force_sample("lon1", false);
        }
        assert_eq!(t.health_of("lon1"), PopHealth::Down);
        t.force_sample("lon1", true);
        t.force_sample("lon1", true);
        assert_eq!(t.health_of("lon1"), PopHealth::Down);
        t.force_sample("lon1", true);
        assert_eq!(t.health_of("lon1"), PopHealth::Up);
    }
}
