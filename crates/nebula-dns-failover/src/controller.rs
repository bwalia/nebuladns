//! Daemon and `--once` control loops.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::api::{
    override_str, pop_health_str, router, token_sha256, ApiState, HostStatus, PopStatus,
    StatusSnapshot,
};
use crate::apply::reconcile_all;
use crate::audit::AuditLog;
use crate::config::Config;
use crate::health::HealthTracker;
use crate::metrics::FailoverMetrics;
use crate::policy::{DesiredTarget, PolicyEngine};
use crate::provider::build_provider;

/// Process exit codes for `--once` (and fatal daemon startup).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    Ok = 0,
    ConfigAuth = 2,
    DnsApplyFailed = 3,
    BothPopsDown = 4,
}

impl From<ExitCode> for u8 {
    fn from(value: ExitCode) -> Self {
        value as Self
    }
}

pub async fn run_once(cfg: Config, audit_path: Option<PathBuf>) -> ExitCode {
    let metrics = FailoverMetrics::new();
    let audit = match AuditLog::new(audit_path) {
        Ok(a) => a,
        Err(err) => {
            error!(error = %err, "audit log open failed");
            return ExitCode::ConfigAuth;
        }
    };
    let provider = match build_provider(&cfg) {
        Ok(p) => p,
        Err(err) => {
            error!(error = %err, "provider init failed");
            return ExitCode::ConfigAuth;
        }
    };
    let mut health = match HealthTracker::new(&cfg) {
        Ok(h) => h,
        Err(err) => {
            error!(error = %err, "health tracker init failed");
            return ExitCode::ConfigAuth;
        }
    };
    let mut policy = PolicyEngine::new(&cfg);

    health.probe_all(&cfg, &metrics).await;
    let summary = reconcile_all(
        &cfg,
        &health,
        &mut policy,
        provider.as_ref(),
        &metrics,
        &audit,
    )
    .await;

    if summary.apply_failed {
        ExitCode::DnsApplyFailed
    } else if summary.both_down {
        ExitCode::BothPopsDown
    } else {
        ExitCode::Ok
    }
}

pub async fn run_daemon(cfg: Config, audit_path: Option<PathBuf>) -> Result<()> {
    let metrics = FailoverMetrics::new();
    let audit = AuditLog::new(audit_path).context("open audit log")?;
    let provider = build_provider(&cfg).context("build provider")?;
    let health = HealthTracker::new(&cfg).context("health tracker")?;
    let policy = Arc::new(RwLock::new(PolicyEngine::new(&cfg)));
    let status = Arc::new(RwLock::new(StatusSnapshot {
        dry_run: cfg.dry_run,
        ready: false,
        pops: Vec::new(),
        hostnames: Vec::new(),
    }));

    let (reconcile_tx, mut reconcile_rx) = tokio::sync::mpsc::channel::<()>(8);

    let api_token_sha256 = cfg.api_token.as_deref().map(token_sha256);
    let api_state = ApiState {
        metrics: metrics.clone(),
        status: status.clone(),
        policy: policy.clone(),
        api_token_sha256,
        reconcile_tx,
    };

    let listen: SocketAddr = cfg
        .listen
        .parse()
        .with_context(|| format!("parse listen {}", cfg.listen))?;
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("bind {listen}"))?;
    info!(%listen, dry_run = cfg.dry_run, "dns-failover listening");

    let app = router(api_state);
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            error!(error = %err, "status HTTP server exited");
        }
    });

    // Provider readiness (non-fatal in dry-run).
    match provider.ready().await {
        Ok(()) => {
            status.write().await.ready = true;
        }
        Err(err) => {
            warn!(error = %err, "provider not ready yet");
            status.write().await.ready = cfg.dry_run;
        }
    }

    let mut health = health;
    let interval = cfg.interval;
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            Some(()) = reconcile_rx.recv() => {
                info!("manual reconcile requested");
            }
        }

        health.probe_all(&cfg, &metrics).await;
        let summary = {
            let mut pol = policy.write().await;
            reconcile_all(&cfg, &health, &mut pol, provider.as_ref(), &metrics, &audit).await
        };
        if summary.both_down {
            warn!("one or more hostnames are fail-open (both POPs down)");
        }
        refresh_status(&cfg, &health, policy.as_ref(), status.as_ref()).await;
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

async fn refresh_status(
    cfg: &Config,
    health: &HealthTracker,
    policy: &RwLock<PolicyEngine>,
    status: &RwLock<StatusSnapshot>,
) {
    let policy = policy.read().await;
    let pops: Vec<PopStatus> = cfg
        .pops
        .keys()
        .map(|id| {
            let h = health.health_of(id);
            PopStatus {
                id: id.clone(),
                health: pop_health_str(h).into(),
                up: health.is_up(id),
            }
        })
        .collect();
    let hostnames: Vec<HostStatus> = cfg
        .hostnames
        .iter()
        .filter(|h| h.enabled)
        .map(|h| HostStatus {
            name: h.name.clone(),
            state: policy.state_of(&h.name).as_str().into(),
            last_transition: policy.last_transition(&h.name).map(str::to_string),
            last_applied: policy.last_applied(&h.name).map(format_desired),
            override_mode: override_str(policy.override_of(&h.name)).into(),
        })
        .collect();
    drop(policy);
    let mut g = status.write().await;
    g.dry_run = cfg.dry_run;
    g.pops = pops;
    g.hostnames = hostnames;
}

fn format_desired(t: &DesiredTarget) -> String {
    match t {
        DesiredTarget::A(ips) => ips
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        DesiredTarget::Cname(c) => format!("CNAME:{c}"),
    }
}
