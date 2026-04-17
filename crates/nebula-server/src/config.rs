//! Configuration loading (TOML).
//!
//! M0 schema covers only the pieces the M0 runtime uses. Full schema arrives with §9 in M5.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    /// DNS listener configuration. When omitted the server does not bind DNS sockets —
    /// only the control plane runs. This is the intended mode for the M0 smoke suite.
    #[serde(default)]
    pub dns: DnsConfig,
    /// Zones to load at startup. Each entry points to a TOML zone file.
    #[serde(default)]
    pub zones: Vec<ZoneConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsConfig {
    /// UDP bind. `None` = UDP disabled.
    pub udp: Option<SocketAddr>,
    /// TCP bind. `None` = TCP disabled.
    pub tcp: Option<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneConfig {
    /// Path to a TOML zone file. Relative paths resolve against the config file's dir.
    pub file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    /// Admin/control-plane HTTP bind. Default: `127.0.0.1:8080`.
    pub bind: SocketAddr,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".parse().unwrap(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    /// `/metrics` bind. Default: `127.0.0.1:9090`. MUST be a different socket from
    /// [`ApiConfig::bind`] — a slow scraper should not starve the control plane.
    pub bind: SocketAddr,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:9090".parse().unwrap(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    /// Tracing filter directive (`RUST_LOG` syntax).
    #[serde(default = "default_log_filter")]
    pub filter: String,
    /// Emit JSON lines on stdout (default: true in containers, false otherwise).
    #[serde(default)]
    pub json: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            filter: default_log_filter(),
            json: true,
        }
    }
}

fn default_log_filter() -> String {
    "info,nebula=debug".to_string()
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

impl Config {
    /// Load a config file from `path`.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Read {
            path: path.to_owned(),
            source: e,
        })?;
        toml::from_str(&text).map_err(|e| ConfigError::Parse {
            path: path.to_owned(),
            source: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrips() {
        let c = Config::default();
        let s = toml::to_string(&c).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.api.bind, c.api.bind);
        assert_eq!(back.metrics.bind, c.metrics.bind);
    }

    #[test]
    fn minimal_toml_parses() {
        let input = r#"
            [api]
            bind = "0.0.0.0:8443"

            [metrics]
            bind = "0.0.0.0:9090"
        "#;
        let c: Config = toml::from_str(input).unwrap();
        assert_eq!(c.api.bind.port(), 8443);
        assert_eq!(c.metrics.bind.port(), 9090);
    }

    #[test]
    fn unknown_field_rejected() {
        let input = r#"
            [api]
            bind = "0.0.0.0:8443"
            bogus = true
        "#;
        assert!(toml::from_str::<Config>(input).is_err());
    }
}
