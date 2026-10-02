//! Cloudflare DNS API provider.
//!
//! Only mutates records whose comment contains the `nebula-dns-failover` marker.
//! Unmarked records on the same name are left untouched.

use std::net::Ipv4Addr;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tracing::{debug, warn};

use crate::config::Config;

use super::{ApplyOutcome, DnsRecord, Provider, ProviderError};

const MARKER: &str = "nebula-dns-failover";
const API: &str = "https://api.cloudflare.com/client/v4";

#[derive(Debug)]
pub struct CloudflareProvider {
    client: Client,
    token: String,
    zone_name: String,
    zone_id: parking_lot::Mutex<Option<String>>,
}

impl CloudflareProvider {
    pub fn new(cfg: &Config) -> Result<Self, ProviderError> {
        let token = cfg
            .provider
            .cf_api_token
            .clone()
            .filter(|t| !t.is_empty())
            .or_else(|| cfg.dry_run.then(|| "dry-run".into()))
            .ok_or_else(|| ProviderError::Auth("CF_API_TOKEN required".into()))?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("nebula-dns-failover/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            token,
            zone_name: cfg.provider.zone_name.trim_end_matches('.').to_string(),
            zone_id: parking_lot::Mutex::new(cfg.provider.zone_id.clone()),
        })
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
    }

    async fn zone_id(&self) -> Result<String, ProviderError> {
        {
            let cached = self.zone_id.lock().clone();
            if let Some(id) = cached {
                return Ok(id);
            }
        }
        let url = format!("{API}/zones?name={}", self.zone_name);
        let resp = self
            .with_retry(|| self.auth(self.client.get(&url)).send())
            .await?;
        let body: CfList<CfZone> = resp.json().await?;
        if !body.success {
            return Err(cf_errors(&body.errors));
        }
        let id = body
            .result
            .into_iter()
            .next()
            .map(|z| z.id)
            .ok_or_else(|| ProviderError::Api(format!("zone {} not found", self.zone_name)))?;
        *self.zone_id.lock() = Some(id.clone());
        Ok(id)
    }

    async fn with_retry<F, Fut>(
        &self,
        mut make: F,
    ) -> Result<reqwest::Response, ProviderError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
    {
        let mut delay = Duration::from_millis(200);
        for attempt in 0..5 {
            let resp = make().await?;
            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                warn!(attempt, "cloudflare rate limited; backing off");
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(2);
                continue;
            }
            return Ok(resp);
        }
        Err(ProviderError::Api("cloudflare rate limit retries exhausted".into()))
    }

    async fn list_raw(&self, name: &str) -> Result<Vec<CfRecord>, ProviderError> {
        let zid = self.zone_id().await?;
        let fqdn = name.trim_end_matches('.');
        let url = format!("{API}/zones/{zid}/dns_records?name={fqdn}");
        let resp = self
            .with_retry(|| self.auth(self.client.get(&url)).send())
            .await?;
        let status = resp.status();
        let body: CfList<CfRecord> = resp.json().await?;
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::Auth(
                "cloudflare auth failed (error 10000?)".into(),
            ));
        }
        if !body.success {
            return Err(cf_errors(&body.errors));
        }
        Ok(body.result)
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_record(
        &self,
        rtype: &str,
        name: &str,
        content: &str,
        ttl: u32,
        comment: &str,
        proxied: bool,
        dry_run: bool,
    ) -> Result<(), ProviderError> {
        if dry_run {
            debug!(rtype, name, content, ttl, "dry-run create");
            return Ok(());
        }
        let zid = self.zone_id().await?;
        let url = format!("{API}/zones/{zid}/dns_records");
        let body = json!({
            "type": rtype,
            "name": name.trim_end_matches('.'),
            "content": content,
            "ttl": ttl,
            "proxied": proxied,
            "comment": comment,
        });
        let resp = self
            .with_retry(|| self.auth(self.client.post(&url).json(&body)).send())
            .await?;
        let parsed: CfList<CfRecord> = resp.json().await?;
        if !parsed.success {
            return Err(cf_errors(&parsed.errors));
        }
        Ok(())
    }

    async fn delete_record(&self, id: &str, dry_run: bool) -> Result<(), ProviderError> {
        if dry_run {
            debug!(id, "dry-run delete");
            return Ok(());
        }
        let zid = self.zone_id().await?;
        let url = format!("{API}/zones/{zid}/dns_records/{id}");
        let resp = self
            .with_retry(|| self.auth(self.client.delete(&url)).send())
            .await?;
        let parsed: CfEnvelope = resp.json().await?;
        if !parsed.success {
            return Err(cf_errors(&parsed.errors));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn update_record(
        &self,
        id: &str,
        rtype: &str,
        name: &str,
        content: &str,
        ttl: u32,
        comment: &str,
        proxied: bool,
        dry_run: bool,
    ) -> Result<(), ProviderError> {
        if dry_run {
            debug!(id, rtype, content, "dry-run update");
            return Ok(());
        }
        let zid = self.zone_id().await?;
        let url = format!("{API}/zones/{zid}/dns_records/{id}");
        let body = json!({
            "type": rtype,
            "name": name.trim_end_matches('.'),
            "content": content,
            "ttl": ttl,
            "proxied": proxied,
            "comment": comment,
        });
        let resp = self
            .with_retry(|| self.auth(self.client.put(&url).json(&body)).send())
            .await?;
        let parsed: CfList<CfRecord> = resp.json().await?;
        if !parsed.success {
            return Err(cf_errors(&parsed.errors));
        }
        Ok(())
    }
}

#[async_trait]
impl Provider for CloudflareProvider {
    async fn ready(&self) -> Result<(), ProviderError> {
        let _ = self.zone_id().await?;
        Ok(())
    }

    async fn get_records(&self, name: &str) -> Result<Vec<DnsRecord>, ProviderError> {
        let raw = self.list_raw(name).await?;
        Ok(raw
            .into_iter()
            .map(|r| DnsRecord {
                id: Some(r.id.clone()),
                name: r.name.clone(),
                rtype: r.type_field.clone(),
                content: r.content.clone(),
                ttl: r.ttl,
                managed: r
                    .comment
                    .as_deref()
                    .is_some_and(|c| c.contains(MARKER)),
            })
            .collect())
    }

    async fn upsert_a(
        &self,
        name: &str,
        ips: &[Ipv4Addr],
        ttl: u32,
        comment: &str,
        proxied: bool,
        dry_run: bool,
    ) -> Result<ApplyOutcome, ProviderError> {
        let all = self.list_raw(name).await?;
        // Never touch unmarked records.
        let managed: Vec<_> = all
            .iter()
            .filter(|r| {
                r.comment
                    .as_deref()
                    .is_some_and(|c| c.contains(MARKER))
            })
            .cloned()
            .collect();
        let foreign: Vec<_> = all
            .iter()
            .filter(|r| {
                !r.comment
                    .as_deref()
                    .is_some_and(|c| c.contains(MARKER))
            })
            .collect();
        if !foreign.is_empty() {
            debug!(
                count = foreign.len(),
                "leaving unmarked Cloudflare records untouched"
            );
        }

        // If switching from managed CNAME → A, delete managed CNAMEs.
        let mut changed = false;
        for r in managed.iter().filter(|r| r.type_field == "CNAME") {
            self.delete_record(&r.id, dry_run).await?;
            changed = true;
        }

        let managed_a: Vec<_> = managed
            .iter()
            .filter(|r| r.type_field == "A")
            .cloned()
            .collect();
        let desired: Vec<String> = ips.iter().map(ToString::to_string).collect();

        // Delete managed A records whose IP is not desired.
        for r in &managed_a {
            if !desired.iter().any(|ip| ip == &r.content) {
                self.delete_record(&r.id, dry_run).await?;
                changed = true;
            }
        }

        // Create missing desired A records.
        for ip in &desired {
            let already = managed_a.iter().any(|r| &r.content == ip && r.ttl == ttl);
            if already {
                continue;
            }
            if let Some(existing) = managed_a.iter().find(|r| &r.content == ip) {
                if existing.ttl != ttl
                    || existing.comment.as_deref() != Some(comment)
                    || existing.proxied != proxied
                {
                    self.update_record(
                        &existing.id,
                        "A",
                        name,
                        ip,
                        ttl,
                        comment,
                        proxied,
                        dry_run,
                    )
                    .await?;
                    changed = true;
                }
            } else {
                self.create_record("A", name, ip, ttl, comment, proxied, dry_run)
                    .await?;
                changed = true;
            }
        }

        if !changed {
            Ok(ApplyOutcome::Noop)
        } else if dry_run {
            Ok(ApplyOutcome::Planned)
        } else {
            Ok(ApplyOutcome::Applied)
        }
    }

    async fn upsert_cname(
        &self,
        name: &str,
        target: &str,
        ttl: u32,
        comment: &str,
        proxied: bool,
        dry_run: bool,
    ) -> Result<ApplyOutcome, ProviderError> {
        let all = self.list_raw(name).await?;
        let managed: Vec<_> = all
            .iter()
            .filter(|r| {
                r.comment
                    .as_deref()
                    .is_some_and(|c| c.contains(MARKER))
            })
            .cloned()
            .collect();

        let mut changed = false;
        // Purge managed A if switching to CNAME.
        for r in managed.iter().filter(|r| r.type_field == "A") {
            self.delete_record(&r.id, dry_run).await?;
            changed = true;
        }

        let want = target.trim_end_matches('.').to_string();
        let cnames: Vec<_> = managed
            .iter()
            .filter(|r| r.type_field == "CNAME")
            .cloned()
            .collect();

        if let Some(existing) = cnames.first() {
            let same = existing.content.trim_end_matches('.') == want
                && existing.ttl == ttl
                && existing.proxied == proxied;
            if same {
                return Ok(if changed {
                    if dry_run {
                        ApplyOutcome::Planned
                    } else {
                        ApplyOutcome::Applied
                    }
                } else {
                    ApplyOutcome::Noop
                });
            }
            // Keep one CNAME: update first, delete extras.
            self.update_record(
                &existing.id,
                "CNAME",
                name,
                &want,
                ttl,
                comment,
                proxied,
                dry_run,
            )
            .await?;
            changed = true;
            for extra in cnames.iter().skip(1) {
                self.delete_record(&extra.id, dry_run).await?;
            }
        } else {
            self.create_record("CNAME", name, &want, ttl, comment, proxied, dry_run)
                .await?;
            changed = true;
        }

        Ok(if !changed {
            ApplyOutcome::Noop
        } else if dry_run {
            ApplyOutcome::Planned
        } else {
            ApplyOutcome::Applied
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct CfList<T> {
    success: bool,
    #[serde(default)]
    errors: Vec<CfError>,
    #[serde(default)]
    result: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct CfEnvelope {
    success: bool,
    #[serde(default)]
    errors: Vec<CfError>,
}

#[derive(Debug, Deserialize)]
struct CfError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CfZone {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CfRecord {
    id: String,
    name: String,
    #[serde(rename = "type")]
    type_field: String,
    content: String,
    ttl: u32,
    #[serde(default)]
    proxied: bool,
    #[serde(default)]
    comment: Option<String>,
}

fn cf_errors(errors: &[CfError]) -> ProviderError {
    let msg = errors
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect::<Vec<_>>()
        .join("; ");
    if errors.iter().any(|e| e.code == 10000) {
        ProviderError::Auth(msg)
    } else {
        ProviderError::Api(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit-level: marker filtering logic used by upsert.
    #[test]
    fn marker_detection() {
        let managed = CfRecord {
            id: "1".into(),
            name: "app.example.com".into(),
            type_field: "A".into(),
            content: "1.2.3.4".into(),
            ttl: 30,
            proxied: false,
            comment: Some("nebula-dns-failover | hostname=app.example.com | policy=lon1-primary | v=1".into()),
        };
        let foreign = CfRecord {
            id: "2".into(),
            name: "app.example.com".into(),
            type_field: "A".into(),
            content: "9.9.9.9".into(),
            ttl: 300,
            proxied: false,
            comment: Some("hand-curated".into()),
        };
        assert!(managed
            .comment
            .as_deref()
            .is_some_and(|c| c.contains(MARKER)));
        assert!(!foreign
            .comment
            .as_deref()
            .is_some_and(|c| c.contains(MARKER)));
    }
}
