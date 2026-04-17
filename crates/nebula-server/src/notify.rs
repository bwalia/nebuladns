//! systemd `sd_notify` integration.
//!
//! On Linux with `Type=notify`, we emit READY=1 after startup and WATCHDOG=1 heartbeats
//! for the duration of `WatchdogSec`. On non-Linux targets this is a no-op.

#[cfg(target_os = "linux")]
pub mod platform {
    use std::time::Duration;

    use sd_notify::NotifyState;
    use tokio::time;

    /// Send READY=1. Call once the server is fully initialized and serving.
    pub fn ready() {
        let _ = sd_notify::notify(false, &[NotifyState::Ready]);
    }

    /// Send STOPPING=1. Call at the start of graceful shutdown.
    pub fn stopping() {
        let _ = sd_notify::notify(false, &[NotifyState::Stopping]);
    }

    /// Spawn a watchdog task if `WATCHDOG_USEC` is set. Sends WATCHDOG=1 every half the
    /// configured interval.
    pub fn spawn_watchdog() -> Option<tokio::task::JoinHandle<()>> {
        let mut usec: u64 = 0;
        if !sd_notify::watchdog_enabled(false, &mut usec) || usec == 0 {
            return None;
        }
        let interval = Duration::from_micros(usec / 2);
        tracing::info!(?interval, "systemd watchdog enabled");
        Some(tokio::spawn(async move {
            let mut ticker = time::interval(interval);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let _ = sd_notify::notify(false, &[NotifyState::Watchdog]);
            }
        }))
    }
}

#[cfg(not(target_os = "linux"))]
pub mod platform {
    pub fn ready() {}
    pub fn stopping() {}
    pub fn spawn_watchdog() -> Option<tokio::task::JoinHandle<()>> {
        None
    }
}
