//! Reconcile loop glue: desire → apply → audit → metrics.

use tracing::{error, warn};

use crate::audit::{unix_ms, ApplyAudit, AuditLog};
use crate::config::Config;
use crate::health::HealthTracker;
use crate::metrics::FailoverMetrics;
use crate::policy::{DesiredTarget, PolicyEngine};
use crate::provider::{apply_desired, ApplyOutcome, Provider};

#[derive(Debug, Default)]
pub struct ReconcileSummary {
    pub both_down: bool,
    pub apply_failed: bool,
    pub applied: u32,
}

pub async fn reconcile_all(
    cfg: &Config,
    health: &HealthTracker,
    policy: &mut PolicyEngine,
    provider: &dyn Provider,
    metrics: &FailoverMetrics,
    audit: &AuditLog,
) -> ReconcileSummary {
    let mut summary = ReconcileSummary::default();
    for host in cfg.hostnames.iter().filter(|h| h.enabled) {
        let desire = policy.desire(cfg, host, health);
        if desire.both_down {
            summary.both_down = true;
            warn!(
                hostname = %host.name,
                reason = %desire.reason,
                "both POPs down — fail-open holding last DNS set"
            );
        }
        metrics.set_state(&host.name, desire.state.as_gauge());

        let comment = Config::marker_comment(&host.name, &host.primary);
        let desired_str = format_desired(&desire.target);

        let outcome = match apply_desired(
            provider,
            &host.name,
            &desire.target,
            cfg.ttl,
            &comment,
            host.proxied,
            cfg.dry_run,
        )
        .await
        {
            Ok(o) => o,
            Err(err) => {
                error!(hostname = %host.name, error = %err, "DNS apply failed");
                metrics.inc_dns_apply(&host.name, "error");
                summary.apply_failed = true;
                audit.emit(&ApplyAudit {
                    ts_unix_ms: unix_ms(),
                    event: "dns_apply",
                    hostname: host.name.clone(),
                    dry_run: cfg.dry_run,
                    reason: desire.reason.clone(),
                    state: desire.state.as_str().into(),
                    desired: desired_str,
                    outcome: format!("error:{err}"),
                    both_down: desire.both_down,
                });
                continue;
            }
        };

        let result_label = match outcome {
            ApplyOutcome::Noop => "noop",
            ApplyOutcome::Planned => "planned",
            ApplyOutcome::Applied => "applied",
        };
        metrics.inc_dns_apply(&host.name, result_label);
        if matches!(outcome, ApplyOutcome::Applied | ApplyOutcome::Planned) {
            summary.applied += 1;
            let from = policy.state_of(&host.name).as_str().to_string();
            metrics.inc_transition(&host.name, &from, desire.state.as_str(), &desire.reason);
        }

        policy.mark_applied(&host.name, desire.target.clone(), desire.state);

        if matches!(outcome, ApplyOutcome::Applied) {
            metrics.touch_last_success();
        }

        audit.emit(&ApplyAudit {
            ts_unix_ms: unix_ms(),
            event: "dns_apply",
            hostname: host.name.clone(),
            dry_run: cfg.dry_run,
            reason: desire.reason,
            state: desire.state.as_str().into(),
            desired: desired_str,
            outcome: result_label.into(),
            both_down: desire.both_down,
        });
    }
    summary
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
