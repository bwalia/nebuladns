//! In-memory hash-chained audit log for zone mutations.
//!
//! Each event's `hash` is SHA-256 over the previous hash and a canonical encoding of
//! the mutation. Tampering with an entry breaks the chain. Persistence to disk is
//! optional; the chain is always kept in process memory for the API to serve.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::auth::{hex_encode, sha256_bytes};

const MAX_EVENTS: usize = 1024;

/// Append-only audit log.
#[derive(Clone, Debug)]
pub struct AuditLog {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    events: Vec<AuditEvent>,
    prev_hash: String,
}

/// One recorded mutation (or refused mutation).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub seq: u64,
    pub ts_unix_ms: u64,
    pub action: String,
    pub zone: String,
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: String,
    pub ttl: Option<u32>,
    pub values: Vec<String>,
    pub dry_run: bool,
    pub result: String,
    pub serial: Option<u32>,
    pub idempotency_key: Option<String>,
    pub prev_hash: String,
    pub hash: String,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                events: Vec::new(),
                prev_hash: "0".repeat(64),
            })),
        }
    }

    /// Append `event`. `hash` / `prev_hash` / `seq` are filled in.
    pub fn append(&self, mut event: AuditEvent) -> AuditEvent {
        let mut inner = self.inner.lock();
        event.seq = inner.events.len() as u64 + 1;
        event.prev_hash.clone_from(&inner.prev_hash);
        event.hash = hash_event(&event);
        inner.prev_hash.clone_from(&event.hash);
        inner.events.push(event.clone());
        if inner.events.len() > MAX_EVENTS {
            inner.events.remove(0);
        }
        event
    }

    #[must_use]
    pub fn list(&self) -> Vec<AuditEvent> {
        self.inner.lock().events.clone()
    }
}

fn hash_event(event: &AuditEvent) -> String {
    // Canonical, field-order-stable encoding. Do not include `hash` itself.
    let canonical = format!(
        "{seq}|{ts}|{action}|{zone}|{name}|{rtype}|{ttl:?}|{values}|{dry}|{result}|{serial:?}|{idem}|{prev}",
        seq = event.seq,
        ts = event.ts_unix_ms,
        action = event.action,
        zone = event.zone,
        name = event.name,
        rtype = event.rtype,
        ttl = event.ttl,
        values = event.values.join(","),
        dry = event.dry_run,
        result = event.result,
        serial = event.serial,
        idem = event.idempotency_key.as_deref().unwrap_or(""),
        prev = event.prev_hash,
    );
    hex_encode(&sha256_bytes(canonical.as_bytes()))
}

pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(action: &str) -> AuditEvent {
        AuditEvent {
            seq: 0,
            ts_unix_ms: 1,
            action: action.into(),
            zone: "example.com.".into(),
            name: "app.example.com.".into(),
            rtype: "CNAME".into(),
            ttl: Some(5),
            values: vec!["west.example.net.".into()],
            dry_run: false,
            result: "ok".into(),
            serial: Some(2),
            idempotency_key: None,
            prev_hash: String::new(),
            hash: String::new(),
        }
    }

    #[test]
    fn chain_links() {
        let log = AuditLog::new();
        let a = log.append(sample("upsert"));
        let b = log.append(sample("upsert"));
        assert_eq!(a.seq, 1);
        assert_eq!(b.seq, 2);
        assert_eq!(b.prev_hash, a.hash);
        assert_ne!(a.hash, b.hash);
        assert_eq!(a.prev_hash, "0".repeat(64));
    }
}
