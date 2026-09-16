//! Bearer-token authentication for the record API.
//!
//! Design:
//! - Fail closed. If no token is configured, every zone/record call is refused.
//! - The configured secret is stored as SHA-256. The request token is hashed and
//!   compared with [`subtle::ConstantTimeEq`] so a timing oracle cannot recover it.
//! - The raw token is never logged.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// Expected token digest. `None` means the control plane refuses zone mutations.
#[derive(Clone)]
pub struct Auth {
    expected_sha256: Option<[u8; 32]>,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth")
            .field("enabled", &self.is_enabled())
            .finish()
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self::disabled()
    }
}

impl Auth {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            expected_sha256: None,
        }
    }

    /// Build from an optional hex digest in config and an optional raw token from the
    /// environment (`NEBULA_API_TOKEN`). If both are present they must agree.
    pub fn from_parts(
        token_sha256_hex: Option<&str>,
        env_token: Option<&str>,
    ) -> Result<Self, AuthSetupError> {
        let from_hex = token_sha256_hex
            .map(parse_sha256_hex)
            .transpose()?
            .flatten();
        let from_env = env_token
            .filter(|t| !t.is_empty())
            .map(|t| sha256_bytes(t.as_bytes()));

        match (from_hex, from_env) {
            (None, None) => Ok(Self::disabled()),
            (Some(hash), None) | (None, Some(hash)) => Ok(Self {
                expected_sha256: Some(hash),
            }),
            (Some(cfg), Some(env)) => {
                if bool::from(cfg.ct_eq(&env)) {
                    Ok(Self {
                        expected_sha256: Some(cfg),
                    })
                } else {
                    Err(AuthSetupError::HashMismatch)
                }
            }
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.expected_sha256.is_some()
    }

    /// Verify a presented bearer token. Always hashes the presented material so the
    /// work factor does not leak whether a token was supplied.
    #[must_use]
    pub fn verify(&self, presented: Option<&str>) -> AuthDecision {
        let Some(expected) = self.expected_sha256 else {
            return AuthDecision::NotConfigured;
        };
        let presented = presented.unwrap_or("");
        let got = sha256_bytes(presented.as_bytes());
        if bool::from(expected.ct_eq(&got)) && !presented.is_empty() {
            AuthDecision::Allow
        } else {
            AuthDecision::Deny
        }
    }
}

/// Outcome of [`Auth::verify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthDecision {
    Allow,
    Deny,
    NotConfigured,
}

/// Startup-time auth configuration errors. These abort the process: running with a
/// misconfigured token is worse than not running.
#[derive(Debug, Error)]
pub enum AuthSetupError {
    #[error("api.token_sha256 must be 64 hex characters")]
    InvalidHash,
    #[error("NEBULA_API_TOKEN does not match api.token_sha256")]
    HashMismatch,
}

pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn parse_sha256_hex(s: &str) -> Result<Option<[u8; 32]>, AuthSetupError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AuthSetupError::InvalidHash);
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|_| AuthSetupError::InvalidHash)?;
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_refuses_everything() {
        let auth = Auth::disabled();
        assert_eq!(auth.verify(Some("secret")), AuthDecision::NotConfigured);
        assert_eq!(auth.verify(None), AuthDecision::NotConfigured);
    }

    #[test]
    fn matching_token_allows() {
        let auth = Auth::from_parts(None, Some("s3cret")).unwrap();
        assert_eq!(auth.verify(Some("s3cret")), AuthDecision::Allow);
        assert_eq!(auth.verify(Some("wrong")), AuthDecision::Deny);
        assert_eq!(auth.verify(None), AuthDecision::Deny);
        assert_eq!(auth.verify(Some("")), AuthDecision::Deny);
    }

    #[test]
    fn hex_and_env_must_agree() {
        let hash = hex_encode(&sha256_bytes(b"s3cret"));
        assert!(Auth::from_parts(Some(&hash), Some("s3cret")).is_ok());
        assert!(matches!(
            Auth::from_parts(Some(&hash), Some("other")),
            Err(AuthSetupError::HashMismatch)
        ));
    }

    #[test]
    fn rejects_malformed_hex() {
        assert!(matches!(
            Auth::from_parts(Some("deadbeef"), None),
            Err(AuthSetupError::InvalidHash)
        ));
    }
}
