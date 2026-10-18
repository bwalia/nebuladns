//! Process signal handling: graceful shutdown (SIGINT / SIGTERM) and zone reload (SIGHUP).

/// Resolves when the process should begin shutdown (SIGINT / SIGTERM).
pub async fn signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal as unix_signal, SignalKind};
        let mut sigterm = unix_signal(SignalKind::terminate()).expect("install SIGTERM handler");
        let mut sigint = unix_signal(SignalKind::interrupt()).expect("install SIGINT handler");
        tokio::select! {
            _ = sigterm.recv() => tracing::info!("received SIGTERM"),
            _ = sigint.recv() => tracing::info!("received SIGINT"),
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("received Ctrl+C");
    }
}

/// SIGHUP stream for zone reloads (`systemctl reload nebuladns`).
///
/// Installing the handler replaces SIGHUP's default action. Without it, a reload
/// terminates the process, and systemd counts a SIGHUP exit as clean, so
/// `Restart=on-failure` would leave DNS down.
#[cfg(unix)]
pub fn hangup() -> std::io::Result<tokio::signal::unix::Signal> {
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
}
