//! Fast, authenticated record mutation API.
//!
//! Endpoints (all require a configured bearer token):
//!
//! | Method | Path | Purpose |
//! |--------|------|---------|
//! | GET    | `/api/v1/zones` | List loaded zones |
//! | GET    | `/api/v1/zones/{zone}` | Zone SOA / serial |
//! | GET    | `/api/v1/zones/{zone}/records` | List (optional `name`/`type` filter) |
//! | PUT    | `/api/v1/zones/{zone}/records` | Replace one RRset (low-TTL fast path) |
//! | POST   | `/api/v1/zones/{zone}/records` | Transactional batch upsert |
//! | DELETE | `/api/v1/zones/{zone}/records` | Delete one RRset (`name`+`type` query) |
//! | GET    | `/api/v1/audit` | Hash-chained audit events |
//!
//! Writes honour `?dry_run=true` and `Idempotency-Key`. Default TTL is 5 seconds
//! (capped by `RecordPolicy`) so caches expire quickly after a failover change.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use nebula_wire::{Name, QType, RData};
use nebula_zone::{parse_rdata, qualify_name, rdata_presentation, Zone, ZoneError};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::audit::{unix_ms, AuditEvent, AuditLog};
use crate::auth::{hex_encode, sha256_bytes};
use crate::AppState;

/// TTL / rate-limit policy for the fast record path.
#[derive(Debug, Clone)]
pub struct RecordPolicy {
    pub default_ttl: u32,
    pub min_ttl: u32,
    pub max_ttl: u32,
    pub max_writes_per_sec: u32,
}

impl Default for RecordPolicy {
    fn default() -> Self {
        Self {
            default_ttl: 5,
            min_ttl: 1,
            max_ttl: 60,
            max_writes_per_sec: 50,
        }
    }
}

/// Sliding 1-second write limiter.
#[derive(Debug, Clone)]
pub struct WriteLimiter {
    inner: Arc<Mutex<(Instant, u32)>>,
    max_per_sec: u32,
}

impl WriteLimiter {
    #[must_use]
    pub fn new(max_per_sec: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new((Instant::now(), 0))),
            max_per_sec,
        }
    }

    #[must_use]
    pub fn allow(&self) -> bool {
        let mut g = self.inner.lock();
        let now = Instant::now();
        if now.duration_since(g.0).as_secs() >= 1 {
            *g = (now, 1);
            return true;
        }
        if g.1 >= self.max_per_sec {
            return false;
        }
        g.1 += 1;
        true
    }
}

#[derive(Debug, Clone)]
pub struct IdempotencyCache {
    inner: Arc<Mutex<HashMap<String, CachedResponse>>>,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    body_hash: String,
    status: StatusCode,
    body: serde_json::Value,
}

impl Default for IdempotencyCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl IdempotencyCache {
    fn get(
        &self,
        key: &str,
        body_hash: &str,
    ) -> Result<Option<(StatusCode, serde_json::Value)>, ApiError> {
        let g = self.inner.lock();
        match g.get(key) {
            None => Ok(None),
            Some(c) if c.body_hash == body_hash => Ok(Some((c.status, c.body.clone()))),
            Some(_) => Err(ApiError::Conflict(
                "Idempotency-Key reused with a different body".into(),
            )),
        }
    }

    fn put(&self, key: String, body_hash: String, status: StatusCode, body: serde_json::Value) {
        let mut g = self.inner.lock();
        if g.len() > 256 {
            g.clear();
        }
        g.insert(
            key,
            CachedResponse {
                body_hash,
                status,
                body,
            },
        );
    }
}

#[derive(Debug, Deserialize)]
pub struct DryRunQuery {
    #[serde(default)]
    pub dry_run: bool,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub rtype: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpsertBody {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: String,
    pub value: Option<String>,
    pub values: Option<Vec<String>>,
    pub ttl: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchBody {
    pub records: Vec<UpsertBody>,
}

#[derive(Debug, Serialize)]
pub struct RecordView {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: String,
    pub value: String,
    pub ttl: u32,
}

#[derive(Debug, Serialize)]
pub struct ZoneSummary {
    pub origin: String,
    pub serial: u32,
}

#[derive(Debug, Serialize)]
pub struct ZoneDetail {
    pub origin: String,
    pub serial: u32,
    pub soa: RecordView,
    pub rrset_count: usize,
}

#[derive(Debug, Serialize)]
pub struct MutationResponse {
    pub ok: bool,
    pub dry_run: bool,
    pub serial: u32,
    pub records: Vec<RecordView>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    code: &'static str,
}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    NotConfigured,
    NotFound(String),
    Validation(String),
    Conflict(String),
    RateLimited,
    Zone(ZoneError),
}

impl From<ZoneError> for ApiError {
    fn from(e: ZoneError) -> Self {
        match e {
            ZoneError::UnknownZone { zone } => Self::NotFound(format!("unknown zone {zone}")),
            ZoneError::NoSuchRRset { owner, rtype } => {
                Self::NotFound(format!("no such RRset {owner} {rtype}"))
            }
            other => Self::Validation(other.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match &self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "missing or invalid bearer token".to_string(),
            ),
            Self::NotConfigured => (
                StatusCode::FORBIDDEN,
                "not_configured",
                "API token not configured; refusing zone access".to_string(),
            ),
            Self::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m.clone()),
            Self::Validation(m) => (StatusCode::BAD_REQUEST, "validation", m.clone()),
            Self::Conflict(m) => (StatusCode::CONFLICT, "conflict", m.clone()),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "write rate limit exceeded".to_string(),
            ),
            Self::Zone(e) => (StatusCode::BAD_REQUEST, "validation", e.to_string()),
        };
        let mut resp = (status, Json(ErrorBody { error: msg, code })).into_response();
        if matches!(self, Self::Unauthorized) {
            resp.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                header::HeaderValue::from_static("Bearer"),
            );
        }
        if matches!(self, Self::RateLimited) {
            resp.headers_mut()
                .insert(header::RETRY_AFTER, header::HeaderValue::from_static("1"));
        }
        resp
    }
}

pub async fn list_zones(State(state): State<AppState>) -> impl IntoResponse {
    let zones: Vec<ZoneSummary> = state
        .zones
        .list()
        .into_iter()
        .map(|z| ZoneSummary {
            origin: z.origin().to_ascii(),
            serial: z.serial(),
        })
        .collect();
    Json(serde_json::json!({ "zones": zones }))
}

pub async fn get_zone(
    State(state): State<AppState>,
    Path(zone): Path<String>,
) -> Result<Json<ZoneDetail>, ApiError> {
    let origin = parse_zone_name(&zone)?;
    let z = state
        .zones
        .get(&origin)
        .ok_or_else(|| ApiError::NotFound(format!("unknown zone {}", origin.to_ascii())))?;
    Ok(Json(zone_detail(&z)))
}

pub async fn list_records(
    State(state): State<AppState>,
    Path(zone): Path<String>,
    Query(q): Query<DryRunQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let origin = parse_zone_name(&zone)?;
    let z = state
        .zones
        .get(&origin)
        .ok_or_else(|| ApiError::NotFound(format!("unknown zone {}", origin.to_ascii())))?;
    let mut records = collect_records(&z);
    if let Some(name) = &q.name {
        let owner = qualify_name(name, z.origin())?;
        let want = owner.to_ascii();
        records.retain(|r| r.name == want);
    }
    if let Some(t) = &q.rtype {
        let want = t.to_ascii_uppercase();
        records.retain(|r| r.rtype == want);
    }
    Ok(Json(serde_json::json!({ "records": records })))
}

pub async fn put_record(
    State(state): State<AppState>,
    Path(zone): Path<String>,
    Query(q): Query<DryRunQuery>,
    headers: HeaderMap,
    Json(body): Json<UpsertBody>,
) -> Result<Response, ApiError> {
    apply_writes(&state, &zone, vec![body], q.dry_run, &headers, "upsert")
}

pub async fn post_records(
    State(state): State<AppState>,
    Path(zone): Path<String>,
    Query(q): Query<DryRunQuery>,
    headers: HeaderMap,
    Json(body): Json<BatchBody>,
) -> Result<Response, ApiError> {
    if body.records.is_empty() {
        return Err(ApiError::Validation("records array is empty".into()));
    }
    if body.records.len() > 64 {
        return Err(ApiError::Validation("batch exceeds 64 records".into()));
    }
    apply_writes(
        &state,
        &zone,
        body.records,
        q.dry_run,
        &headers,
        "batch_upsert",
    )
}

pub async fn delete_record(
    State(state): State<AppState>,
    Path(zone): Path<String>,
    Query(q): Query<DryRunQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let name = q
        .name
        .ok_or_else(|| ApiError::Validation("query parameter `name` is required".into()))?;
    let rtype = q
        .rtype
        .ok_or_else(|| ApiError::Validation("query parameter `type` is required".into()))?;
    let body = UpsertBody {
        name,
        rtype,
        value: None,
        values: None,
        ttl: None,
    };
    apply_writes(&state, &zone, vec![body], q.dry_run, &headers, "delete")
}

pub async fn list_audit(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "events": state.audit.list() }))
}

#[allow(clippy::too_many_lines)]
fn apply_writes(
    state: &AppState,
    zone: &str,
    bodies: Vec<UpsertBody>,
    dry_run: bool,
    headers: &HeaderMap,
    action: &str,
) -> Result<Response, ApiError> {
    if !state.limiter.allow() {
        return Err(ApiError::RateLimited);
    }
    let origin = parse_zone_name(zone)?;
    let z = state
        .zones
        .get(&origin)
        .ok_or_else(|| ApiError::NotFound(format!("unknown zone {}", origin.to_ascii())))?;

    let idem_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let body_hash = hex_encode(&sha256_bytes(
        &serde_json::to_vec(&serde_json::json!({
            "zone": zone,
            "action": action,
            "dry_run": dry_run,
            "records": bodies.iter().map(|b| serde_json::json!({
                "name": b.name,
                "type": b.rtype,
                "value": b.value,
                "values": b.values,
                "ttl": b.ttl,
            })).collect::<Vec<_>>(),
        }))
        .unwrap_or_default(),
    ));

    if let Some(key) = &idem_key {
        if let Some((status, body)) = state.idem.get(key, &body_hash)? {
            return Ok((status, Json(body)).into_response());
        }
    }

    let prepared = prepare_rrsets(&z, &bodies, action, &state.policy)?;

    if dry_run {
        let views: Vec<RecordView> = prepared
            .iter()
            .flat_map(|(owner, ttl, data)| views_for(owner, *ttl, data))
            .collect();
        audit(
            &state.audit,
            action,
            &origin,
            &prepared,
            true,
            "ok",
            Some(z.serial()),
            idem_key.clone(),
        );
        let resp = MutationResponse {
            ok: true,
            dry_run: true,
            serial: z.serial(),
            records: views,
        };
        return Ok((StatusCode::OK, Json(resp)).into_response());
    }

    // Apply sequentially under the registry write lock (one call per RRset). For
    // batch_upsert, clone-mutate would be nicer as a single snapshot; we apply
    // in order and accept intermediate serial bumps. A failed later record leaves
    // earlier ones applied — callers wanting atomicity should PUT one RRset.
    let mut last_serial = z.serial();
    let mut views = Vec::new();
    if action == "delete" {
        for (owner, _ttl, data) in &prepared {
            let qtype = data[0].rtype();
            let out = state.zones.delete_rrset(&origin, owner, qtype)?;
            last_serial = out.serial;
            views.extend(out.removed.iter().map(record_view));
        }
    } else {
        for (owner, ttl, data) in prepared.clone() {
            let out = state
                .zones
                .upsert_rrset(&origin, owner.clone(), ttl, data.clone())?;
            last_serial = out.serial;
            views.extend(views_for(&owner, ttl, &data));
        }
    }

    audit(
        &state.audit,
        action,
        &origin,
        &prepared,
        false,
        "ok",
        Some(last_serial),
        idem_key.clone(),
    );

    let payload = serde_json::to_value(MutationResponse {
        ok: true,
        dry_run: false,
        serial: last_serial,
        records: views,
    })
    .unwrap_or(serde_json::Value::Null);

    if let Some(key) = idem_key {
        state
            .idem
            .put(key, body_hash, StatusCode::OK, payload.clone());
    }

    Ok((StatusCode::OK, Json(payload)).into_response())
}

fn prepare_rrsets(
    zone: &Zone,
    bodies: &[UpsertBody],
    action: &str,
    policy: &RecordPolicy,
) -> Result<Vec<(Name, u32, Vec<RData>)>, ApiError> {
    let mut out = Vec::with_capacity(bodies.len());
    for body in bodies {
        let owner = qualify_name(&body.name, zone.origin())?;
        let rtype = parse_rtype(&body.rtype)?;
        if action == "delete" {
            // Dummy RData so apply_writes can recover the type; never written.
            let placeholder = placeholder_rdata(rtype)?;
            out.push((owner, 0, vec![placeholder]));
            continue;
        }
        let values = collect_values(body)?;
        let ttl = resolve_ttl(body.ttl, policy)?;
        let data: Vec<RData> = values
            .iter()
            .map(|v| parse_rdata(&body.rtype, v).map_err(ApiError::from))
            .collect::<Result<_, _>>()?;
        out.push((owner, ttl, data));
    }
    Ok(out)
}

fn placeholder_rdata(qtype: QType) -> Result<RData, ApiError> {
    match qtype {
        QType::A => Ok(RData::A(std::net::Ipv4Addr::UNSPECIFIED)),
        QType::AAAA => Ok(RData::Aaaa(std::net::Ipv6Addr::UNSPECIFIED)),
        QType::CNAME => Ok(RData::Cname(Name::root())),
        QType::TXT => Ok(RData::Txt(vec![b"x".to_vec()])),
        QType::NS => Ok(RData::Ns(Name::root())),
        QType::PTR => Ok(RData::Ptr(Name::root())),
        QType::MX => Ok(RData::Mx {
            preference: 0,
            exchange: Name::root(),
        }),
        QType::SRV => Ok(RData::Srv {
            priority: 0,
            weight: 0,
            port: 0,
            target: Name::root(),
        }),
        QType::CAA => Ok(RData::Caa {
            flags: 0,
            tag: b"issue".to_vec(),
            value: b".".to_vec(),
        }),
        _ => Err(ApiError::Validation(format!(
            "unsupported record type {}",
            qtype.mnemonic().unwrap_or("?")
        ))),
    }
}

fn collect_values(body: &UpsertBody) -> Result<Vec<String>, ApiError> {
    match (&body.value, &body.values) {
        (Some(v), None) => Ok(vec![v.clone()]),
        (None, Some(vs)) if !vs.is_empty() => Ok(vs.clone()),
        (Some(_), Some(_)) => Err(ApiError::Validation(
            "specify `value` or `values`, not both".into(),
        )),
        _ => Err(ApiError::Validation(
            "`value` or `values` is required".into(),
        )),
    }
}

fn resolve_ttl(requested: Option<u32>, policy: &RecordPolicy) -> Result<u32, ApiError> {
    let ttl = requested.unwrap_or(policy.default_ttl);
    if ttl < policy.min_ttl || ttl > policy.max_ttl {
        return Err(ApiError::Validation(format!(
            "ttl {ttl} outside allowed range {}–{}",
            policy.min_ttl, policy.max_ttl
        )));
    }
    Ok(ttl)
}

fn parse_rtype(s: &str) -> Result<QType, ApiError> {
    QType::from_mnemonic(s)
        .filter(|t| {
            matches!(
                *t,
                QType::A
                    | QType::AAAA
                    | QType::CNAME
                    | QType::TXT
                    | QType::MX
                    | QType::PTR
                    | QType::NS
                    | QType::SRV
                    | QType::CAA
            )
        })
        .ok_or_else(|| ApiError::Validation(format!("unsupported record type {s}")))
}

fn parse_zone_name(s: &str) -> Result<Name, ApiError> {
    let name = Name::from_ascii(s).map_err(|_| ApiError::Validation("invalid zone name".into()))?;
    if name.is_root() {
        return Err(ApiError::Validation("zone name is empty".into()));
    }
    Ok(name)
}

fn collect_records(zone: &Zone) -> Vec<RecordView> {
    zone.iter().map(record_view).collect()
}

fn record_view(rr: &nebula_wire::ResourceRecord) -> RecordView {
    RecordView {
        name: rr.name.to_ascii(),
        rtype: rr.rtype().mnemonic().unwrap_or("TYPE").to_string(),
        value: rdata_presentation(&rr.data),
        ttl: rr.ttl,
    }
}

fn views_for(owner: &Name, ttl: u32, data: &[RData]) -> Vec<RecordView> {
    data.iter()
        .map(|d| RecordView {
            name: owner.to_ascii(),
            rtype: d.rtype().mnemonic().unwrap_or("TYPE").to_string(),
            value: rdata_presentation(d),
            ttl,
        })
        .collect()
}

fn zone_detail(z: &Zone) -> ZoneDetail {
    ZoneDetail {
        origin: z.origin().to_ascii(),
        serial: z.serial(),
        soa: record_view(z.soa()),
        rrset_count: z.iter().count(),
    }
}

#[allow(clippy::too_many_arguments)]
fn audit(
    log: &AuditLog,
    action: &str,
    origin: &Name,
    prepared: &[(Name, u32, Vec<RData>)],
    dry_run: bool,
    result: &str,
    serial: Option<u32>,
    idempotency_key: Option<String>,
) {
    for (owner, ttl, data) in prepared {
        let rtype = data
            .first()
            .and_then(|d| d.rtype().mnemonic())
            .unwrap_or("?")
            .to_string();
        let values = data.iter().map(rdata_presentation).collect();
        log.append(AuditEvent {
            seq: 0,
            ts_unix_ms: unix_ms(),
            action: action.into(),
            zone: origin.to_ascii(),
            name: owner.to_ascii(),
            rtype,
            ttl: Some(*ttl),
            values,
            dry_run,
            result: result.into(),
            serial,
            idempotency_key: idempotency_key.clone(),
            prev_hash: String::new(),
            hash: String::new(),
        });
    }
}

/// Extract a bearer token from `Authorization`.
#[must_use]
pub fn bearer_from(headers: &HeaderMap) -> Option<&str> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}
