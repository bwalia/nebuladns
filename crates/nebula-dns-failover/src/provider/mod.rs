//! Pluggable DNS providers.

mod cloudflare;
mod nebuladns;

use std::net::Ipv4Addr;

use async_trait::async_trait;
use thiserror::Error;

use crate::config::{Config, ProviderKind};
use crate::policy::DesiredTarget;

pub use cloudflare::CloudflareProvider;
pub use nebuladns::NebulaDnsProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsRecord {
    pub id: Option<String>,
    pub name: String,
    pub rtype: String,
    pub content: String,
    pub ttl: u32,
    /// Provider-specific management marker (Cloudflare comment, etc.).
    pub managed: bool,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api: {0}")]
    Api(String),
    #[error("auth: {0}")]
    Auth(String),
    #[error("{0}")]
    Other(String),
}

#[async_trait]
pub trait Provider: Send + Sync {
    async fn ready(&self) -> Result<(), ProviderError>;

    async fn get_records(&self, name: &str) -> Result<Vec<DnsRecord>, ProviderError>;

    async fn upsert_a(
        &self,
        name: &str,
        ips: &[Ipv4Addr],
        ttl: u32,
        comment: &str,
        proxied: bool,
        dry_run: bool,
    ) -> Result<ApplyOutcome, ProviderError>;

    async fn upsert_cname(
        &self,
        name: &str,
        target: &str,
        ttl: u32,
        comment: &str,
        proxied: bool,
        dry_run: bool,
    ) -> Result<ApplyOutcome, ProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    Noop,
    Planned,
    Applied,
}

pub fn build_provider(cfg: &Config) -> Result<Box<dyn Provider>, ProviderError> {
    match cfg.provider.kind {
        ProviderKind::Nebuladns => Ok(Box::new(NebulaDnsProvider::new(cfg)?)),
        ProviderKind::Cloudflare => Ok(Box::new(CloudflareProvider::new(cfg)?)),
    }
}

pub async fn apply_desired(
    provider: &dyn Provider,
    name: &str,
    desired: &DesiredTarget,
    ttl: u32,
    comment: &str,
    proxied: bool,
    dry_run: bool,
) -> Result<ApplyOutcome, ProviderError> {
    match desired {
        DesiredTarget::A(ips) => {
            provider
                .upsert_a(name, ips, ttl, comment, proxied, dry_run)
                .await
        }
        DesiredTarget::Cname(target) => {
            provider
                .upsert_cname(name, target, ttl, comment, proxied, dry_run)
                .await
        }
    }
}
