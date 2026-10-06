//! MCP tool catalogue.
//!
//! Each tool is a named entry in `catalogue()` with a JSON-Schema-shaped input contract
//! and a dispatch arm in `invoke`. The split between read and write tools is load-bearing
//! — writes are refused unless the operator set `NEBULA_MCP_ALLOW_WRITES=1`.
//!
//! Tools whose HTTP endpoint is not implemented in the server yet are flagged with an
//! `[M5]` prefix in their description so the model (and the human reading the tool list)
//! knows to expect a 404 until the milestone ships.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::ApiClient;
use crate::protocol::{ToolCallResult, ToolDescriptor};

/// API clients the tools talk to. `nebula-dns-failover` is a separate service with its
/// own address and bearer token, so it gets its own client.
#[derive(Debug, Clone)]
pub struct Clients {
    pub api: ApiClient,
    pub failover: ApiClient,
}

/// Returns the static tool catalogue advertised via `tools/list`.
///
/// Kept as a single function despite length: splitting this into per-category helpers
/// fragments what is effectively a flat data declaration and makes the full surface
/// harder to audit in one place.
#[allow(clippy::too_many_lines)]
pub fn catalogue() -> Vec<ToolDescriptor> {
    vec![
        // -------- read-only, available today (M0/M1) --------
        ToolDescriptor {
            name: "health_check",
            description: "Probe /livez and /readyz on the NebulaDNS admin API. Returns \
                          the HTTP status for each.",
            input_schema: empty_schema(),
        },
        ToolDescriptor {
            name: "get_version",
            description: "Fetch NebulaDNS server version, commit, rustc, and target from \
                          /api/v1/version.",
            input_schema: empty_schema(),
        },
        ToolDescriptor {
            name: "get_metrics",
            description: "Fetch Prometheus text exposition from /metrics. Useful for \
                          inspecting per-zone query counts, rcodes, and transfer state.",
            input_schema: empty_schema(),
        },
        // -------- read-only, M5-gated --------
        ToolDescriptor {
            name: "list_zones",
            description: "List all loaded zones (origin + SOA serial). Requires a bearer token.",
            input_schema: empty_schema(),
        },
        ToolDescriptor {
            name: "get_zone",
            description: "Fetch a single zone's origin, SOA serial, and RRset count. Requires a bearer token.",
            input_schema: zone_name_schema(),
        },
        ToolDescriptor {
            name: "get_zone_history",
            description: "[M5] Return the version history and diffs for a zone.",
            input_schema: zone_name_schema(),
        },
        ToolDescriptor {
            name: "get_propagation_status",
            description: "[M5] Return per-secondary propagation state (last SOA serial \
                          observed, lag, FORMERR count).",
            input_schema: zone_name_schema(),
        },
        ToolDescriptor {
            name: "list_secondaries",
            description: "[M5] List all configured secondary servers and their health.",
            input_schema: empty_schema(),
        },
        ToolDescriptor {
            name: "get_dnssec_status",
            description: "[M5] Return DNSSEC key state, next rollover, and NSEC params \
                          for a zone.",
            input_schema: zone_name_schema(),
        },
        ToolDescriptor {
            name: "get_deploy_status",
            description: "[M5] Return status for a deploy id returned by `deploy`.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "deploy_id": { "type": "string", "description": "Deploy identifier." }
                },
                "required": ["deploy_id"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "list_tsig_keys",
            description: "[M5] List TSIG keys (names and algorithms only; secret material \
                          is never returned).",
            input_schema: empty_schema(),
        },
        // -------- mutating (write-gated) --------
        ToolDescriptor {
            name: "create_zone",
            description: "[M5][write] Create a new zone. Requires NEBULA_MCP_ALLOW_WRITES=1.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "zone": { "type": "object", "description": "Zone document (SOA + initial records)." }
                },
                "required": ["zone"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "replace_zone",
            description: "[M5][write] Replace a zone's full record set. Requires \
                          NEBULA_MCP_ALLOW_WRITES=1.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Zone apex (FQDN, trailing dot optional)." },
                    "zone": { "type": "object", "description": "New zone document." }
                },
                "required": ["name", "zone"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "add_records",
            description: "[write] Batch upsert records in a zone (transactional per request, \
                          low TTL by default). Requires NEBULA_MCP_ALLOW_WRITES=1 and a bearer token.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "records": { "type": "array", "items": { "type": "object" } }
                },
                "required": ["name", "records"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "set_record",
            description: "[write] Fast upsert of a single RRset (CNAME/A/AAAA/TXT/…). \
                          Defaults to TTL 5. Requires NEBULA_MCP_ALLOW_WRITES=1 and a bearer token.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Zone apex." },
                    "record_name": { "type": "string", "description": "Owner name (`@`, relative, or FQDN)." },
                    "type": { "type": "string", "description": "RR type, e.g. CNAME." },
                    "value": { "type": "string" },
                    "ttl": { "type": "integer", "minimum": 1, "description": "TTL seconds. Default 5, max 60." },
                    "dry_run": { "type": "boolean", "default": false }
                },
                "required": ["name", "record_name", "type", "value"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "delete_record",
            description: "[write] Delete one RRset by owner name and type. Requires \
                          NEBULA_MCP_ALLOW_WRITES=1 and a bearer token.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "record_name": { "type": "string" },
                    "type": { "type": "string" },
                    "dry_run": { "type": "boolean", "default": false }
                },
                "required": ["name", "record_name", "type"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "get_audit_log",
            description: "Return the hash-chained audit log of zone mutations. Requires a bearer token.",
            input_schema: empty_schema(),
        },
        ToolDescriptor {
            name: "rollback_zone",
            description: "[M5][write] Roll a zone back to a prior version. Requires \
                          NEBULA_MCP_ALLOW_WRITES=1.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "version": { "type": "string", "description": "Target version identifier from get_zone_history." }
                },
                "required": ["name", "version"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "force_notify",
            description: "[M5][write] Force-send DNS NOTIFY to all secondaries for a \
                          zone. Requires NEBULA_MCP_ALLOW_WRITES=1.",
            input_schema: zone_name_schema(),
        },
        ToolDescriptor {
            name: "trigger_dnssec_rollover",
            description: "[M5][write] Trigger a DNSSEC key rollover. Requires \
                          NEBULA_MCP_ALLOW_WRITES=1.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "key_type": { "type": "string", "enum": ["ksk", "zsk"] }
                },
                "required": ["name", "key_type"],
                "additionalProperties": false,
            }),
        },
        ToolDescriptor {
            name: "deploy",
            description: "[M5][write] Trigger a deploy. Returns a deploy id usable with \
                          get_deploy_status. Requires NEBULA_MCP_ALLOW_WRITES=1.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "wait_for_propagation": { "type": "boolean", "default": false },
                    "timeout_seconds": { "type": "integer", "minimum": 1 }
                },
                "additionalProperties": false,
            }),
        },
        // -------- DNS failover controller (nebula-dns-failover) --------
        ToolDescriptor {
            name: "failover_status",
            description: "Snapshot from the nebula-dns-failover controller: per-POP health \
                          (up/down/unknown — unknown means hysteresis hasn't settled yet), \
                          and per-hostname serving state, last applied target, last \
                          transition, and any manual override. Also reports dry_run.",
            input_schema: empty_schema(),
        },
        ToolDescriptor {
            name: "failover_force_secondary",
            description: "[write] Pin a hostname to its secondary POP. The override is \
                          STICKY: health-driven failover/failback is disabled for this \
                          hostname until failover_clear_override is called. Confirm with the \
                          operator first. Requires NEBULA_MCP_ALLOW_WRITES=1 and \
                          NEBULA_FAILOVER_TOKEN.",
            input_schema: hostname_schema(),
        },
        ToolDescriptor {
            name: "failover_force_primary",
            description: "[write] Pin a hostname to its primary POP (sticky, like \
                          failover_force_secondary). Confirm with the operator first. \
                          Requires NEBULA_MCP_ALLOW_WRITES=1 and NEBULA_FAILOVER_TOKEN.",
            input_schema: hostname_schema(),
        },
        ToolDescriptor {
            name: "failover_clear_override",
            description: "[write] Remove a manual override so health-driven policy decides \
                          again, then reconcile. Requires NEBULA_MCP_ALLOW_WRITES=1 and \
                          NEBULA_FAILOVER_TOKEN.",
            input_schema: hostname_schema(),
        },
        ToolDescriptor {
            name: "failover_reconcile",
            description: "[write] Run a probe + reconcile cycle now instead of waiting for \
                          the next interval. Changes no policy, but may apply DNS if the \
                          desired target differs. Requires NEBULA_MCP_ALLOW_WRITES=1 and \
                          NEBULA_FAILOVER_TOKEN.",
            input_schema: hostname_schema(),
        },
    ]
}

/// Dispatch a tool call. Returns a `ToolCallResult` ready to be serialised.
///
/// The `allow_writes` flag is a server-level gate: without it, mutating tools refuse up
/// front so we never hit the API at all. Refusal is surfaced as a tool-level error
/// (`isError: true`) so the model sees it and can explain to the user what to do.
#[allow(clippy::too_many_lines)]
pub async fn invoke(
    name: &str,
    args: &Value,
    clients: &Clients,
    allow_writes: bool,
) -> Result<ToolCallResult> {
    let client = &clients.api;
    let args = if args.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        args.clone()
    };

    match name {
        // ---- read-only, M0/M1 ----
        "health_check" => health_check(client).await,
        "get_version" => get_version(client).await,
        "get_metrics" => get_metrics(client).await,
        // ---- read-only, M5 ----
        "list_zones" => api_get_json(client, "/api/v1/zones").await,
        "get_zone" => api_get_json(client, &zone_path(&args, "", None)?).await,
        "get_audit_log" => api_get_json(client, "/api/v1/audit").await,
        "get_zone_history" => api_get_json(client, &zone_path(&args, "/history", None)?).await,
        "get_propagation_status" => {
            api_get_json(client, &zone_path(&args, "/propagation", None)?).await
        }
        "list_secondaries" => api_get_json(client, "/api/v1/secondaries").await,
        "get_dnssec_status" => api_get_json(client, &zone_path(&args, "/dnssec", None)?).await,
        "get_deploy_status" => {
            let id = str_field(&args, "deploy_id")?;
            api_get_json(client, &format!("/api/v1/deploys/{id}")).await
        }
        "list_tsig_keys" => api_get_json(client, "/api/v1/keys/tsig").await,
        // ---- mutating ----
        "create_zone" => {
            guard_write(allow_writes)?;
            let body = args
                .get("zone")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing `zone`"))?;
            api_post_json(client, "/api/v1/zones", &body).await
        }
        "replace_zone" => {
            guard_write(allow_writes)?;
            let path = zone_path(&args, "", None)?;
            let body = args
                .get("zone")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing `zone`"))?;
            api_put_json(client, &path, &body).await
        }
        "add_records" => {
            guard_write(allow_writes)?;
            let path = zone_path(&args, "/records", None)?;
            let records = args
                .get("records")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing `records`"))?;
            api_post_json(client, &path, &json!({ "records": records })).await
        }
        "set_record" => {
            guard_write(allow_writes)?;
            let dry = args
                .get("dry_run")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let path = if dry {
                format!("{}?dry_run=true", zone_path(&args, "/records", None)?)
            } else {
                zone_path(&args, "/records", None)?
            };
            let body = json!({
                "name": str_field(&args, "record_name")?,
                "type": str_field(&args, "type")?,
                "value": str_field(&args, "value")?,
                "ttl": args.get("ttl").and_then(Value::as_u64),
            });
            api_put_json(client, &path, &body).await
        }
        "delete_record" => {
            guard_write(allow_writes)?;
            let record_name = str_field(&args, "record_name")?;
            let rtype = str_field(&args, "type")?;
            let dry = args
                .get("dry_run")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut path = format!(
                "{}?name={}&type={}",
                zone_path(&args, "/records", None)?,
                record_name,
                rtype
            );
            if dry {
                path.push_str("&dry_run=true");
            }
            api_delete_json(client, &path).await
        }
        "rollback_zone" => {
            guard_write(allow_writes)?;
            let path = zone_path(&args, "/rollback", None)?;
            let version = str_field(&args, "version")?;
            api_post_json(client, &path, &json!({ "version": version })).await
        }
        "force_notify" => {
            guard_write(allow_writes)?;
            let path = zone_path(&args, "/notify", None)?;
            api_post_json(client, &path, &Value::Null).await
        }
        "trigger_dnssec_rollover" => {
            guard_write(allow_writes)?;
            let path = zone_path(&args, "/dnssec/rollover", None)?;
            let key_type = str_field(&args, "key_type")?;
            api_post_json(client, &path, &json!({ "key_type": key_type })).await
        }
        "deploy" => {
            guard_write(allow_writes)?;
            api_post_json(client, "/api/v1/deploy", &args).await
        }
        // ---- failover controller ----
        "failover_status" => api_get_json(&clients.failover, "/v1/status").await,
        "failover_force_secondary" => {
            failover_action(clients, &args, "failover", allow_writes).await
        }
        "failover_force_primary" => failover_action(clients, &args, "failback", allow_writes).await,
        "failover_clear_override" => failover_action(clients, &args, "auto", allow_writes).await,
        "failover_reconcile" => failover_action(clients, &args, "reconcile", allow_writes).await,
        other => Ok(ToolCallResult::error(format!("unknown tool: {other}"))),
    }
}

// ---- helpers ----------------------------------------------------------------

fn empty_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

fn zone_name_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "Zone apex (FQDN). Trailing dot optional."
            }
        },
        "required": ["name"],
        "additionalProperties": false,
    })
}

fn hostname_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "hostname": {
                "type": "string",
                "description": "Managed hostname (FQDN) as listed by failover_status."
            }
        },
        "required": ["hostname"],
        "additionalProperties": false,
    })
}

fn str_field(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("missing or non-string `{key}`"))
}

/// Build `/api/v1/zones/<name><suffix>`.
fn zone_path(args: &Value, suffix: &str, _version: Option<&str>) -> Result<String> {
    let name = dns_name_field(args, "name")?;
    Ok(format!("/api/v1/zones/{name}{suffix}"))
}

/// Read a DNS name argument for use as a URL path segment. Names are RFC-1035 labels
/// joined by dots, so only dots, alphanumerics, `-` and `_` appear — reject anything
/// else up front rather than shipping a malformed (or path-traversing) URL.
fn dns_name_field(args: &Value, key: &str) -> Result<String> {
    let name = str_field(args, key)?;
    let name = name.trim_end_matches('.');
    if name.is_empty() {
        anyhow::bail!("`{key}` is empty");
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        anyhow::bail!("`{key}` contains invalid characters: {name}");
    }
    Ok(name.to_string())
}

fn guard_write(allow_writes: bool) -> Result<()> {
    if allow_writes {
        Ok(())
    } else {
        anyhow::bail!("this tool mutates DNS state; set NEBULA_MCP_ALLOW_WRITES=1 to enable it")
    }
}

/// `POST /v1/hostnames/<hostname>/<action>` on the failover controller.
async fn failover_action(
    clients: &Clients,
    args: &Value,
    action: &str,
    allow_writes: bool,
) -> Result<ToolCallResult> {
    guard_write(allow_writes)?;
    let hostname = dns_name_field(args, "hostname")?;
    api_post_json(
        &clients.failover,
        &format!("/v1/hostnames/{hostname}/{action}"),
        &Value::Null,
    )
    .await
}

async fn health_check(client: &ApiClient) -> Result<ToolCallResult> {
    let livez = client.get_text("/livez").await?;
    let readyz = client.get_text("/readyz").await?;
    Ok(ToolCallResult::text(format!(
        "livez: {} ({})\nreadyz: {} ({})",
        livez.status,
        livez.body.trim(),
        readyz.status,
        readyz.body.trim(),
    )))
}

async fn get_version(client: &ApiClient) -> Result<ToolCallResult> {
    let body = client.get_json("/api/v1/version").await?;
    Ok(ToolCallResult::text(serde_json::to_string_pretty(&body)?))
}

async fn get_metrics(client: &ApiClient) -> Result<ToolCallResult> {
    let resp = client.get_text("/metrics").await?;
    resp.expect_success()?;
    Ok(ToolCallResult::text(resp.body))
}

async fn api_get_json(client: &ApiClient, path: &str) -> Result<ToolCallResult> {
    let body = client.get_json(path).await?;
    Ok(ToolCallResult::text(serde_json::to_string_pretty(&body)?))
}

async fn api_post_json(client: &ApiClient, path: &str, body: &Value) -> Result<ToolCallResult> {
    let resp = client.post_json(path, body).await?;
    Ok(ToolCallResult::text(serde_json::to_string_pretty(&resp)?))
}

async fn api_put_json(client: &ApiClient, path: &str, body: &Value) -> Result<ToolCallResult> {
    let resp = client.put_json(path, body).await?;
    Ok(ToolCallResult::text(serde_json::to_string_pretty(&resp)?))
}

async fn api_delete_json(client: &ApiClient, path: &str) -> Result<ToolCallResult> {
    let resp = client.delete(path).await?;
    Ok(ToolCallResult::text(serde_json::to_string_pretty(&resp)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_has_expected_tools() {
        let names: Vec<&str> = catalogue().iter().map(|t| t.name).collect();
        for expected in [
            "health_check",
            "get_version",
            "get_metrics",
            "list_zones",
            "get_zone",
            "create_zone",
            "deploy",
            "set_record",
            "delete_record",
            "get_audit_log",
            "failover_status",
            "failover_force_secondary",
            "failover_force_primary",
            "failover_clear_override",
            "failover_reconcile",
        ] {
            assert!(names.contains(&expected), "missing tool: {expected}");
        }
    }

    #[test]
    fn write_gate_blocks_without_env() {
        assert!(guard_write(false).is_err());
        assert!(guard_write(true).is_ok());
    }

    #[test]
    fn zone_path_rejects_bad_chars() {
        let args = json!({"name": "evil zone"});
        assert!(zone_path(&args, "", None).is_err());
        let args = json!({"name": "../../etc/passwd"});
        assert!(zone_path(&args, "", None).is_err());
    }

    #[test]
    fn zone_path_strips_trailing_dot() {
        let args = json!({"name": "example.com."});
        let p = zone_path(&args, "/history", None).unwrap();
        assert_eq!(p, "/api/v1/zones/example.com/history");
    }

    #[tokio::test]
    async fn failover_writes_refused_without_gate() {
        // Unroutable clients: the gate must refuse before any connection attempt.
        let unreachable = ApiClient::new("127.0.0.1:0".parse().unwrap(), None);
        let clients = Clients {
            api: unreachable.clone(),
            failover: unreachable,
        };
        let args = json!({"hostname": "abtesting.fictionally.org"});
        for tool in [
            "failover_force_secondary",
            "failover_force_primary",
            "failover_clear_override",
            "failover_reconcile",
        ] {
            let err = invoke(tool, &args, &clients, false).await.unwrap_err();
            assert!(
                err.to_string().contains("NEBULA_MCP_ALLOW_WRITES"),
                "{tool}: {err}"
            );
        }
    }

    #[test]
    fn failover_hostname_rejects_path_traversal() {
        for bad in ["../status", "a/b", "evil host", ""] {
            assert!(
                dns_name_field(&json!({"hostname": bad}), "hostname").is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            dns_name_field(
                &json!({"hostname": "abtesting.fictionally.org."}),
                "hostname"
            )
            .unwrap(),
            "abtesting.fictionally.org"
        );
    }

    #[test]
    fn zone_path_requires_name() {
        let args = json!({});
        assert!(zone_path(&args, "", None).is_err());
    }
}
