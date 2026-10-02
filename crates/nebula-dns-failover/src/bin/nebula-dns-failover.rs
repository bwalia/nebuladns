//! `nebula-dns-failover` — health-aware DNS failover controller.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode as StdExit;

use anyhow::{Context, Result};
use clap::Parser;
use nebula_dns_failover::{run_daemon, run_once, Config, ExitCode};
use tracing::info;

#[derive(Debug, Parser)]
#[command(
    name = "nebula-dns-failover",
    version,
    about = "Probe edge POPs and rewrite managed DNS records for failover"
)]
struct Cli {
    /// Path to TOML config (see deploy/dns-failover/config.example.toml).
    #[arg(long, env = "NEBULA_DNS_FAILOVER_CONFIG")]
    config: PathBuf,

    /// Run a single probe+reconcile cycle and exit.
    #[arg(long)]
    once: bool,

    /// Force dry-run for this invocation (overrides config).
    #[arg(long)]
    dry_run: bool,

    /// Optional NDJSON audit file (default: stdout only via tracing).
    #[arg(long, env = "NEBULA_DNS_FAILOVER_AUDIT")]
    audit_log: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> StdExit {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    match try_main().await {
        Ok(code) => StdExit::from(u8::from(code)),
        Err(err) => {
            eprintln!("fatal: {err:#}");
            StdExit::from(u8::from(ExitCode::ConfigAuth))
        }
    }
}

async fn try_main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let mut cfg = Config::load(&cli.config)
        .with_context(|| format!("load config {}", cli.config.display()))?;
    if cli.dry_run {
        cfg.dry_run = true;
    }
    info!(
        config = %cli.config.display(),
        dry_run = cfg.dry_run,
        once = cli.once,
        provider = ?cfg.provider.kind,
        "starting nebula-dns-failover"
    );

    if cli.once {
        Ok(run_once(cfg, cli.audit_log).await)
    } else {
        run_daemon(cfg, cli.audit_log).await?;
        Ok(ExitCode::Ok)
    }
}
