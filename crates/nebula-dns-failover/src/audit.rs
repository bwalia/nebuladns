//! NDJSON audit log for every probe decision and DNS apply.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tracing::info;

#[derive(Debug)]
pub struct AuditLog {
    file: Option<Mutex<std::fs::File>>,
}

impl AuditLog {
    pub fn new(path: Option<PathBuf>) -> std::io::Result<Self> {
        let file = match path {
            Some(p) => {
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let f = OpenOptions::new().create(true).append(true).open(p)?;
                Some(Mutex::new(f))
            }
            None => None,
        };
        Ok(Self { file })
    }

    pub fn emit<T: Serialize>(&self, event: &T) {
        let Ok(line) = serde_json::to_string(event) else {
            return;
        };
        info!(target: "nebula_dns_failover::audit", "{line}");
        if let Some(file) = &self.file {
            if let Ok(mut g) = file.lock() {
                let _ = writeln!(g, "{line}");
                let _ = g.flush();
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApplyAudit {
    pub ts_unix_ms: u64,
    pub event: &'static str,
    pub hostname: String,
    pub dry_run: bool,
    pub reason: String,
    pub state: String,
    pub desired: String,
    pub outcome: String,
    pub both_down: bool,
}

pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
