//! Atomic in-memory zone registry.
//!
//! Readers load a snapshot through `arc-swap` and never block. Writers take a mutex,
//! clone the affected zone, mutate it, and publish a new snapshot. Lost-update races
//! between two writers are serialised; a DNS query in flight may observe either the
//! old or the new snapshot, never a torn RRset.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use nebula_wire::{Name, QType, RData};
use parking_lot::Mutex;

use crate::mutate::{DeleteOutcome, UpsertOutcome};
use crate::{Zone, ZoneError};

/// Shared registry of loaded zones, keyed by lowercased origin.
#[derive(Clone)]
pub struct ZoneRegistry {
    inner: Arc<ArcSwap<HashMap<Name, Arc<Zone>>>>,
    write: Arc<Mutex<()>>,
}

impl Default for ZoneRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ZoneRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.inner.load().len();
        f.debug_struct("ZoneRegistry")
            .field("zones", &n)
            .finish_non_exhaustive()
    }
}

impl ZoneRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ArcSwap::new(Arc::new(HashMap::new()))),
            write: Arc::new(Mutex::new(())),
        }
    }

    /// Replace the registry contents atomically.
    pub fn replace(&self, zones: impl IntoIterator<Item = Zone>) {
        let _guard = self.write.lock();
        let map: HashMap<Name, Arc<Zone>> = zones
            .into_iter()
            .map(|z| (z.origin().to_ascii_lowercase(), Arc::new(z)))
            .collect();
        self.inner.store(Arc::new(map));
    }

    /// Snapshot of every loaded zone.
    #[must_use]
    pub fn list(&self) -> Vec<Arc<Zone>> {
        self.inner.load().values().cloned().collect()
    }

    /// Exact origin lookup (trailing-dot / case insensitive).
    #[must_use]
    pub fn get(&self, origin: &Name) -> Option<Arc<Zone>> {
        self.inner.load().get(&origin.to_ascii_lowercase()).cloned()
    }

    /// Find the most-specific zone containing `qname`.
    #[must_use]
    pub fn zone_for(&self, qname: &Name) -> Option<Arc<Zone>> {
        let map = self.inner.load();
        let qname_lower = qname.to_ascii_lowercase();
        let labels = qname_lower.labels();
        for skip in 0..=labels.len() {
            let tail = labels[skip..].to_vec();
            let Ok(candidate) = Name::from_labels(tail) else {
                continue;
            };
            if let Some(z) = map.get(&candidate) {
                return Some(z.clone());
            }
        }
        None
    }

    /// Upsert an RRset in `origin`. Fails if the zone is not loaded.
    pub fn upsert_rrset(
        &self,
        origin: &Name,
        owner: Name,
        ttl: u32,
        data: Vec<RData>,
    ) -> Result<UpsertOutcome, ZoneError> {
        self.mutate(origin, |zone| zone.upsert_rrset(owner, ttl, data))
    }

    /// Delete an RRset in `origin`.
    pub fn delete_rrset(
        &self,
        origin: &Name,
        owner: &Name,
        qtype: QType,
    ) -> Result<DeleteOutcome, ZoneError> {
        self.mutate(origin, |zone| zone.delete_rrset(owner, qtype))
    }

    fn mutate<T>(
        &self,
        origin: &Name,
        f: impl FnOnce(&mut Zone) -> Result<T, ZoneError>,
    ) -> Result<T, ZoneError> {
        let origin_key = origin.to_ascii_lowercase();
        let _guard = self.write.lock();
        let current = self.inner.load();
        let zone = current
            .get(&origin_key)
            .ok_or_else(|| ZoneError::UnknownZone {
                zone: origin.to_ascii(),
            })?;
        let mut next_zone = (**zone).clone();
        let result = f(&mut next_zone)?;
        let mut next_map = (**current).clone();
        next_map.insert(origin_key, Arc::new(next_zone));
        self.inner.store(Arc::new(next_map));
        Ok(result)
    }
}
