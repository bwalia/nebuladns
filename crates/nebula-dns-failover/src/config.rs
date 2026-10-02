//! YAML/TOML-shaped configuration for the failover controller.
//!
//! Config is TOML (NebulaDNS convention). Duration fields accept `"10s"`, `"2m"`,
//! or integer seconds.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("{0}")]
    Validation(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Bind for `/livez`, `/readyz`, `/metrics`, `/v1/status`.
    #[serde(default = "default_listen")]
    pub listen: String,
    /// When true, log planned DNS writes but never call the provider mutate path.
    #[serde(default)]
    pub dry_run: bool,
    #[serde(
        default = "default_interval",
        deserialize_with = "deserialize_duration"
    )]
    pub interval: Duration,
    #[serde(
        default = "default_probe_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub probe_timeout: Duration,
    #[serde(default = "default_consecutive_fail")]
    pub consecutive_fail: u32,
    #[serde(default = "default_consecutive_ok")]
    pub consecutive_ok: u32,
    /// Optional soak before failback after primary recovers.
    #[serde(default, deserialize_with = "deserialize_opt_duration")]
    pub failback_delay: Option<Duration>,
    #[serde(default = "default_ttl")]
    pub ttl: u32,
    /// `all` (default) or `any` — how multiple health URLs combine.
    #[serde(default = "default_require")]
    pub require: RequireMode,
    pub provider: ProviderConfig,
    pub pops: BTreeMap<String, PopConfig>,
    pub hostnames: Vec<HostnameConfig>,
    /// Optional shared bearer for mutating `/v1/hostnames/...` endpoints.
    /// Env `FAILOVER_API_TOKEN` overrides when set.
    #[serde(default)]
    pub api_token: Option<String>,
}

fn default_listen() -> String {
    "127.0.0.1:9119".into()
}
fn default_interval() -> Duration {
    Duration::from_secs(10)
}
fn default_probe_timeout() -> Duration {
    Duration::from_secs(3)
}
fn default_consecutive_fail() -> u32 {
    3
}
fn default_consecutive_ok() -> u32 {
    3
}
fn default_ttl() -> u32 {
    30
}
fn default_require() -> RequireMode {
    RequireMode::All
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequireMode {
    All,
    Any,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub kind: ProviderKind,
    /// NebulaDNS admin API base, e.g. `http://127.0.0.1:8080`.
    #[serde(default)]
    pub api_base: Option<String>,
    /// Bearer for NebulaDNS record API (`NEBULA_API_TOKEN` env overrides).
    #[serde(default)]
    pub api_token: Option<String>,
    /// Zone origin without trailing dot, e.g. `fictionally.org`.
    pub zone_name: String,
    /// Cloudflare zone id (optional; resolved from `zone_name` when absent).
    #[serde(default)]
    pub zone_id: Option<String>,
    /// Cloudflare API token (`CF_API_TOKEN` env overrides).
    #[serde(default)]
    pub cf_api_token: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Nebuladns,
    Cloudflare,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopConfig {
    #[serde(default)]
    pub display_name: Option<String>,
    pub public_ipv4: Ipv4Addr,
    pub health_urls: Vec<String>,
    /// Only for raw-IP probes with a Host header / broken certs. Document the risk.
    #[serde(default)]
    pub insecure_skip_verify: bool,
    #[serde(default)]
    pub host_header: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostnameConfig {
    pub name: String,
    #[serde(default = "default_mode")]
    pub mode: FailoverMode,
    pub primary: String,
    pub secondary: String,
    #[serde(default = "default_record_type")]
    pub record_type: RecordType,
    /// CNAME targets keyed by POP id when `record_type = cname`.
    #[serde(default)]
    pub cname_targets: BTreeMap<String, String>,
    /// Cloudflare orange-cloud. Prefer false for edge IP failover.
    #[serde(default)]
    pub proxied: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_mode() -> FailoverMode {
    FailoverMode::ActivePassive
}
fn default_record_type() -> RecordType {
    RecordType::A
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FailoverMode {
    ActivePassive,
    ActiveActive,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum RecordType {
    A,
    Cname,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        let mut cfg: Self = toml::from_str(&text)?;
        cfg.apply_env_overrides();
        cfg.validate()?;
        Ok(cfg)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(t) = std::env::var("FAILOVER_API_TOKEN") {
            if !t.is_empty() {
                self.api_token = Some(t);
            }
        }
        if let Ok(t) = std::env::var("NEBULA_API_TOKEN") {
            if !t.is_empty() {
                self.provider.api_token = Some(t);
            }
        }
        if let Ok(t) = std::env::var("CF_API_TOKEN") {
            if !t.is_empty() {
                self.provider.cf_api_token = Some(t);
            }
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.pops.is_empty() {
            return Err(ConfigError::Validation("pops must not be empty".into()));
        }
        if self.consecutive_fail == 0 || self.consecutive_ok == 0 {
            return Err(ConfigError::Validation(
                "consecutive_fail and consecutive_ok must be >= 1".into(),
            ));
        }
        if self.ttl == 0 {
            return Err(ConfigError::Validation("ttl must be >= 1".into()));
        }
        match self.provider.kind {
            ProviderKind::Nebuladns => {
                if self
                    .provider
                    .api_base
                    .as_ref()
                    .map_or(true, String::is_empty)
                {
                    return Err(ConfigError::Validation(
                        "provider.api_base required for nebuladns provider".into(),
                    ));
                }
            }
            ProviderKind::Cloudflare => {
                if self
                    .provider
                    .cf_api_token
                    .as_ref()
                    .map_or(true, String::is_empty)
                {
                    return Err(ConfigError::Validation(
                        "CF_API_TOKEN or provider.cf_api_token required for cloudflare".into(),
                    ));
                }
            }
        }
        for h in &self.hostnames {
            if !h.enabled {
                continue;
            }
            if !self.pops.contains_key(&h.primary) {
                return Err(ConfigError::Validation(format!(
                    "hostname {} primary pop '{}' not in pops",
                    h.name, h.primary
                )));
            }
            if !self.pops.contains_key(&h.secondary) {
                return Err(ConfigError::Validation(format!(
                    "hostname {} secondary pop '{}' not in pops",
                    h.name, h.secondary
                )));
            }
            if h.record_type == RecordType::Cname {
                for pop in [&h.primary, &h.secondary] {
                    if !h.cname_targets.contains_key(pop) {
                        return Err(ConfigError::Validation(format!(
                            "hostname {} missing cname_targets.{pop}",
                            h.name
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn marker_comment(hostname: &str, primary: &str) -> String {
        format!("nebula-dns-failover | hostname={hostname} | policy={primary}-primary | v=1")
    }
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = TomlDuration::deserialize(deserializer)?;
    Ok(v.0)
}

fn deserialize_opt_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<TomlDuration>::deserialize(deserializer)?;
    Ok(opt.map(|d| d.0))
}

#[derive(Debug, Clone)]
struct TomlDuration(Duration);

impl<'de> Deserialize<'de> for TomlDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        struct V;
        impl Visitor<'_> for V {
            type Value = TomlDuration;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a duration string like \"10s\" or an integer number of seconds")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(TomlDuration(Duration::from_secs(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                let n = u64::try_from(v).map_err(|_| E::custom("duration must be non-negative"))?;
                Ok(TomlDuration(Duration::from_secs(n)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                parse_duration(v).map(TomlDuration).map_err(E::custom)
            }
        }
        deserializer.deserialize_any(V)
    }
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".into());
    }
    if let Ok(secs) = s.parse::<u64>() {
        return Ok(Duration::from_secs(secs));
    }
    if let Some(num) = s.strip_suffix("ms") {
        let n: u64 = num.parse().map_err(|_| format!("invalid duration {s:?}"))?;
        return Ok(Duration::from_millis(n));
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: u64 = num
        .parse()
        .map_err(|_| format!("invalid duration number in {s:?}"))?;
    match unit {
        "s" => Ok(Duration::from_secs(n)),
        "m" => Ok(Duration::from_secs(n * 60)),
        "h" => Ok(Duration::from_secs(n * 3600)),
        _ => Err(format!("unknown duration unit in {s:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_config() {
        let toml = r#"
listen = "127.0.0.1:9119"
dry_run = true
interval = "10s"
probe_timeout = "3s"
consecutive_fail = 3
consecutive_ok = 3
ttl = 30

[provider]
type = "nebuladns"
api_base = "http://127.0.0.1:8080"
zone_name = "fictionally.org"

[pops.lon1]
display_name = "London lon1"
public_ipv4 = "195.20.255.201"
health_urls = ["https://lon1.pop0.uk/healthz"]

[pops.lon2]
display_name = "London lon2"
public_ipv4 = "85.190.106.189"
health_urls = ["https://lon2.pop0.uk/healthz"]

[[hostnames]]
name = "abtesting.fictionally.org"
mode = "active-passive"
primary = "lon1"
secondary = "lon2"
record_type = "A"
enabled = true
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        cfg.validate().unwrap();
        assert_eq!(cfg.interval, Duration::from_secs(10));
        assert!(cfg.dry_run);
        assert_eq!(cfg.pops.len(), 2);
    }

    #[test]
    fn rejects_unknown_primary() {
        let toml = r#"
[provider]
type = "nebuladns"
api_base = "http://127.0.0.1:8080"
zone_name = "example.com"
[pops.lon1]
public_ipv4 = "195.20.255.201"
health_urls = ["https://lon1.example/healthz"]
[[hostnames]]
name = "app.example.com"
primary = "missing"
secondary = "lon1"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert!(cfg.validate().is_err());
    }
}
