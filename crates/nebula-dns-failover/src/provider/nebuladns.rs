//! NebulaDNS fast record API provider.
//!
//! Ownership model: hostnames listed in the failover config are fully managed.
//! The provider replaces the A or CNAME RRset for that owner via
//! `PUT /api/v1/zones/{zone}/records` and never touches other owners.
//! Fail-closed: requires `NEBULA_API_TOKEN` / `provider.api_token`.
//!
//! Each hostname may override `provider.zone_name` via `hostname.zone_name`
//! when managing records across multiple delegated zones.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::config::Config;

use super::{ApplyOutcome, DnsRecord, Provider, ProviderError};

#[derive(Debug)]
pub struct NebulaDnsProvider {
    client: Client,
    base: String,
    token: String,
    default_zone: String,
    /// Lowercased FQDN (no trailing dot) → zone origin (no trailing dot).
    zones_by_host: HashMap<String, String>,
}

impl NebulaDnsProvider {
    pub fn new(cfg: &Config) -> Result<Self, ProviderError> {
        let base = cfg
            .provider
            .api_base
            .clone()
            .ok_or_else(|| ProviderError::Other("api_base missing".into()))?
            .trim_end_matches('/')
            .to_string();
        // Live applies are fail-closed. Dry-run may use a placeholder so local
        // smoke (`--once --dry-run`) works without a real token or API.
        let token = cfg
            .provider
            .api_token
            .clone()
            .filter(|t| !t.is_empty())
            .or_else(|| cfg.dry_run.then(|| "dry-run".into()))
            .ok_or_else(|| {
                ProviderError::Auth(
                    "NEBULA_API_TOKEN or provider.api_token required (fail-closed)".into(),
                )
            })?;
        let default_zone = cfg.provider.zone_name.trim_end_matches('.').to_string();
        let mut zones_by_host = HashMap::new();
        for h in &cfg.hostnames {
            let zone = h
                .zone_name
                .as_deref()
                .unwrap_or(default_zone.as_str())
                .trim_end_matches('.')
                .to_string();
            let key = h.name.trim_end_matches('.').to_ascii_lowercase();
            zones_by_host.insert(key, zone);
        }
        let client = Client::builder()
            .user_agent(concat!("nebula-dns-failover/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            base,
            token,
            default_zone,
            zones_by_host,
        })
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
    }

    fn zone_for(&self, fqdn: &str) -> &str {
        let key = fqdn.trim_end_matches('.').to_ascii_lowercase();
        self.zones_by_host
            .get(&key)
            .map(String::as_str)
            .unwrap_or(self.default_zone.as_str())
    }

    fn owner_relative(&self, fqdn: &str) -> String {
        let fqdn = fqdn.trim_end_matches('.');
        let zone = self.zone_for(fqdn);
        if fqdn.eq_ignore_ascii_case(zone) {
            "@".into()
        } else if let Some(prefix) = fqdn
            .to_ascii_lowercase()
            .strip_suffix(&format!(".{}", zone.to_ascii_lowercase()))
        {
            prefix.to_string()
        } else {
            fqdn.to_string()
        }
    }
}

/// A fresh `Idempotency-Key` per apply attempt.
///
/// The record API keeps keys with no expiry: a reused key with a different body is
/// rejected (409), and one with the same body is replayed without writing. A key
/// derived from the owner (or its target) therefore broke every failover after the
/// first write, or would silently skip a failback. The controller re-reads the live
/// record every tick, so it doesn't need a key that is stable across ticks.
fn idempotency_key(kind: &str, owner: &str) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("failover-{kind}-{owner}-{nanos}-{seq}")
}

#[async_trait]
impl Provider for NebulaDnsProvider {
    async fn ready(&self) -> Result<(), ProviderError> {
        let url = format!("{}/readyz", self.base);
        let resp = self.client.get(&url).send().await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::Api(format!(
                "readyz returned {}",
                resp.status()
            )))
        }
    }

    async fn get_records(&self, name: &str) -> Result<Vec<DnsRecord>, ProviderError> {
        let zone = self.zone_for(name);
        let owner = self.owner_relative(name);
        let url = format!(
            "{}/api/v1/zones/{}/records?name={}",
            self.base,
            urlencoding_zone(zone),
            owner
        );
        let resp = self.auth(self.client.get(&url)).send().await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(ProviderError::Auth(format!(
                "list records: {}",
                resp.status()
            )));
        }
        if !resp.status().is_success() {
            return Err(ProviderError::Api(format!(
                "list records {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )));
        }
        let body: serde_json::Value = resp.json().await?;
        let mut out = Vec::new();
        if let Some(arr) = body.get("records").and_then(|v| v.as_array()) {
            for r in arr {
                out.push(DnsRecord {
                    id: None,
                    name: r
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    rtype: r
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    content: r
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    ttl: r
                        .get("ttl")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|v| u32::try_from(v).ok())
                        .unwrap_or(0),
                    // Config ownership is the marker for NebulaDNS.
                    managed: true,
                });
            }
        }
        Ok(out)
    }

    async fn upsert_a(
        &self,
        name: &str,
        ips: &[Ipv4Addr],
        ttl: u32,
        _comment: &str,
        _proxied: bool,
        dry_run: bool,
    ) -> Result<ApplyOutcome, ProviderError> {
        if dry_run {
            tracing::info!(
                hostname = %name,
                ips = ?ips,
                ttl,
                "dry-run: would upsert A via NebulaDNS record API"
            );
            return Ok(ApplyOutcome::Planned);
        }
        let zone = self.zone_for(name);
        let owner = self.owner_relative(name);
        let existing = self.get_records(name).await?;
        let mut current: Vec<String> = existing
            .iter()
            .filter(|r| r.rtype.eq_ignore_ascii_case("A"))
            .map(|r| r.content.trim_end_matches('.').to_string())
            .collect();
        current.sort();
        let mut desired: Vec<String> = ips.iter().map(ToString::to_string).collect();
        desired.sort();
        let ttl_match = existing
            .iter()
            .filter(|r| r.rtype.eq_ignore_ascii_case("A"))
            .all(|r| r.ttl == ttl)
            || existing
                .iter()
                .filter(|r| r.rtype.eq_ignore_ascii_case("A"))
                .count()
                == 0;
        if current == desired && ttl_match && !desired.is_empty() {
            return Ok(ApplyOutcome::Noop);
        }

        let url = format!(
            "{}/api/v1/zones/{}/records",
            self.base,
            urlencoding_zone(zone)
        );
        let body = json!({
            "name": owner,
            "type": "A",
            "values": desired,
            "ttl": ttl,
        });
        let resp = self
            .auth(self.client.put(&url).json(&body))
            .header("Idempotency-Key", idempotency_key("a", &owner))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ProviderError::Api(format!(
                "upsert A {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )));
        }
        Ok(ApplyOutcome::Applied)
    }

    async fn upsert_cname(
        &self,
        name: &str,
        target: &str,
        ttl: u32,
        _comment: &str,
        _proxied: bool,
        dry_run: bool,
    ) -> Result<ApplyOutcome, ProviderError> {
        if dry_run {
            tracing::info!(
                hostname = %name,
                target,
                ttl,
                "dry-run: would upsert CNAME via NebulaDNS record API"
            );
            return Ok(ApplyOutcome::Planned);
        }
        let zone = self.zone_for(name);
        let owner = self.owner_relative(name);
        let existing = self.get_records(name).await?;
        let want = normalize_cname(target);
        if let Some(cur) = existing
            .iter()
            .find(|r| r.rtype.eq_ignore_ascii_case("CNAME"))
        {
            if normalize_cname(&cur.content) == want && cur.ttl == ttl {
                return Ok(ApplyOutcome::Noop);
            }
        }
        let url = format!(
            "{}/api/v1/zones/{}/records",
            self.base,
            urlencoding_zone(zone)
        );
        let body = json!({
            "name": owner,
            "type": "CNAME",
            "value": want,
            "ttl": ttl,
        });
        let resp = self
            .auth(self.client.put(&url).json(&body))
            .header("Idempotency-Key", idempotency_key("cname", &owner))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ProviderError::Api(format!(
                "upsert CNAME {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )));
        }
        Ok(ApplyOutcome::Applied)
    }
}

fn normalize_cname(s: &str) -> String {
    let s = s.trim();
    if s.ends_with('.') {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

fn urlencoding_zone(zone: &str) -> String {
    // Zone names are DNS labels; keep simple percent for safety.
    zone.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_relative_strips_zone() {
        let p = NebulaDnsProvider {
            client: Client::new(),
            base: "http://127.0.0.1:8080".into(),
            token: "t".into(),
            default_zone: "fictionally.org".into(),
            zones_by_host: HashMap::from([(
                "abtesting.fictionally.org".into(),
                "fictionally.org".into(),
            )]),
        };
        assert_eq!(p.owner_relative("abtesting.fictionally.org"), "abtesting");
        assert_eq!(p.owner_relative("fictionally.org."), "@");
    }

    #[test]
    fn owner_relative_apex_of_delegated_child() {
        let p = NebulaDnsProvider {
            client: Client::new(),
            base: "http://127.0.0.1:8080".into(),
            token: "t".into(),
            default_zone: "failover.nebuladns.net".into(),
            zones_by_host: HashMap::from([(
                "abtesting.fictionally.org".into(),
                "abtesting.fictionally.org".into(),
            )]),
        };
        assert_eq!(p.owner_relative("abtesting.fictionally.org"), "@");
        assert_eq!(p.zone_for("abtesting.fictionally.org"), "abtesting.fictionally.org");
    }
}
