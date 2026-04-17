//! Top-level runtime wiring for `nebuladns`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use nebula_api::{control_plane_router, metrics_router, AppState};
use nebula_metrics::{dns::DnsMetrics, Metrics};
use nebula_zone::Zone;
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::{
    config::{Config, ZoneConfig},
    dns::{serve_tcp, serve_udp, ZoneRegistry},
    notify::platform as notify,
    shutdown, telemetry,
};

/// CLI arguments for the `nebuladns` binary.
#[derive(Debug, Parser)]
#[command(
    name = "nebuladns",
    version,
    about = "NebulaDNS authoritative DNS server"
)]
pub struct Args {
    /// Path to the TOML config file. Defaults look in `/etc/nebuladns/nebuladns.toml`.
    #[arg(long, short, env = "NEBULA_CONFIG")]
    pub config: Option<PathBuf>,

    /// Emit a default config to stdout and exit.
    #[arg(long)]
    pub print_default_config: bool,

    /// Healthcheck mode: `GET /livez` against the admin bind and exit with 0/1.
    ///
    /// Used as the container HEALTHCHECK — `nebuladns healthcheck`.
    #[arg(long, hide = true)]
    pub healthcheck: bool,
}

/// Main entry. Owns the runtime and returns only on shutdown.
#[allow(clippy::too_many_lines)]
pub async fn main(args: Args) -> Result<()> {
    if args.print_default_config {
        let cfg = Config::default();
        print!("{}", toml::to_string_pretty(&cfg)?);
        return Ok(());
    }

    if args.healthcheck {
        return healthcheck().await;
    }

    let cfg_path = args.config.as_deref();
    let cfg = load_config(cfg_path)?;
    telemetry::init(&cfg.logging);

    let metrics = Metrics::global();
    metrics.set_build_info(
        env!("CARGO_PKG_VERSION"),
        option_env!("NEBULA_GIT_COMMIT").unwrap_or("unknown"),
        option_env!("NEBULA_RUSTC_VERSION").unwrap_or("unknown"),
        option_env!("NEBULA_TARGET").unwrap_or(std::env::consts::ARCH),
    );

    // Register data-plane metrics into the global registry (always-on per §6 contract).
    let dns_metrics = metrics.with_registry_mut(DnsMetrics::register);

    // Load zones declared in config and publish them into the registry.
    let zone_registry = ZoneRegistry::new();
    let zones = load_zones(cfg_path, &cfg.zones)?;
    if !zones.is_empty() {
        tracing::info!(count = zones.len(), "loaded zones");
    }
    zone_registry.replace(zones);

    let state = AppState::new(metrics.clone());

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        api.bind = %cfg.api.bind,
        metrics.bind = %cfg.metrics.bind,
        "starting nebuladns",
    );

    let control_listener = TcpListener::bind(cfg.api.bind)
        .await
        .with_context(|| format!("bind control plane on {}", cfg.api.bind))?;
    let metrics_listener = TcpListener::bind(cfg.metrics.bind)
        .await
        .with_context(|| format!("bind metrics on {}", cfg.metrics.bind))?;

    let control_app = control_plane_router(state.clone());
    let metrics_app = metrics_router(state.clone());

    let mut servers: JoinSet<Result<()>> = JoinSet::new();
    let shutdown_token = CancellationToken::new();

    // DNS listeners (optional).
    if let Some(udp_addr) = cfg.dns.udp {
        let zones = zone_registry.clone();
        let metrics = dns_metrics.clone();
        let token = shutdown_token.clone();
        servers.spawn(async move {
            serve_udp(udp_addr, zones, metrics, token)
                .await
                .context("udp listener failed")
        });
    }
    if let Some(tcp_addr) = cfg.dns.tcp {
        let zones = zone_registry.clone();
        let metrics = dns_metrics.clone();
        let token = shutdown_token.clone();
        servers.spawn(async move {
            serve_tcp(tcp_addr, zones, metrics, token)
                .await
                .context("tcp listener failed")
        });
    }

    {
        let token = shutdown_token.clone();
        servers.spawn(async move {
            axum::serve(control_listener, control_app)
                .with_graceful_shutdown(async move { token.cancelled().await })
                .await
                .context("control-plane server failed")
        });
    }
    {
        let token = shutdown_token.clone();
        servers.spawn(async move {
            axum::serve(metrics_listener, metrics_app)
                .with_graceful_shutdown(async move { token.cancelled().await })
                .await
                .context("metrics server failed")
        });
    }

    // All subsystems are up — signal ready.
    state.set_ready(true);
    notify::ready();
    let _watchdog = notify::spawn_watchdog();
    tracing::info!("nebuladns ready");

    shutdown::signal().await;
    notify::stopping();
    state.set_ready(false);
    tracing::info!("shutting down");
    shutdown_token.cancel();

    while let Some(res) = servers.join_next().await {
        match res {
            Ok(Ok(())) => {}
            Ok(Err(err)) => tracing::error!(error = ?err, "server task failed"),
            Err(err) => tracing::error!(error = %err, "server task panicked"),
        }
    }

    tracing::info!("nebuladns stopped cleanly");
    Ok(())
}

fn load_zones(cfg_path: Option<&Path>, zones: &[ZoneConfig]) -> Result<Vec<Zone>> {
    let base = cfg_path
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    zones
        .iter()
        .map(|z| {
            let path = if z.file.is_absolute() {
                z.file.clone()
            } else {
                base.join(&z.file)
            };
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read zone file {}", path.display()))?;
            Zone::from_toml(&text).with_context(|| format!("parse zone file {}", path.display()))
        })
        .collect()
}

fn load_config(explicit: Option<&std::path::Path>) -> Result<Config> {
    let path = if let Some(p) = explicit {
        Some(p.to_path_buf())
    } else {
        let default = PathBuf::from("/etc/nebuladns/nebuladns.toml");
        if default.exists() {
            Some(default)
        } else {
            None
        }
    };

    match path {
        Some(p) => Config::load(&p).with_context(|| format!("load config from {}", p.display())),
        None => {
            // No config: run with defaults (useful for `docker run ... --demo` and CI).
            Ok(Config::default())
        }
    }
}

/// `nebuladns healthcheck` implementation: GET /livez on localhost. Used as the container
/// HEALTHCHECK so the image needs no extra tooling (curl, wget).
async fn healthcheck() -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::{timeout, Duration};

    let bind = std::env::var("NEBULA_API_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let do_probe = async {
        let mut stream = TcpStream::connect(&bind).await.context("connect")?;
        let req = b"GET /livez HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        stream.write_all(req).await.context("write request")?;
        let mut first = [0u8; 16];
        let n = stream.read(&mut first).await.context("read status")?;
        if n < 12 {
            anyhow::bail!("short response: {n} bytes");
        }
        // "HTTP/1.1 200" or "HTTP/1.0 200"
        if &first[9..12] != b"200" {
            anyhow::bail!("non-200 status: {:?}", &first[..n]);
        }
        Ok::<(), anyhow::Error>(())
    };
    timeout(Duration::from_secs(2), do_probe)
        .await
        .context("healthcheck timed out")?
        .with_context(|| format!("livez probe failed for {bind}"))?;
    println!("ok");
    Ok(())
}
