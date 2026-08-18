//! Mock OAuth 2.0 authorization server: RFC 6749 four-grant coverage
//! (authorization_code + PKCE, implicit, password, refresh_token rotation,
//! client_credentials) plus a Bearer-protected `/oauth2/userinfo`.
//!
//! Design contract: `internal/oauth2-endpoints-design.md`. Path tree follows the conventional
//! `/oauth2/authorize|token|userinfo` layout for drop-in migration; the
//! self-describing index and RFC 8414 discovery document are httpcan extras.
//!
//! State model (mock semantics, documented in `GET /oauth2`):
//! - Codes and tokens are `payload.mac` envelopes — `payload` is compact JSON,
//!   `mac` is HMAC-SHA256 over it with a per-process random key. Nothing
//!   client-forgeable; restarting the process invalidates everything issued.
//! - One-time semantics (RFC 6749 §4.1.2 MUST for codes; rotation for refresh
//!   tokens) use a process-global replay cache keyed by token fingerprint.
//! - No client registry: any `client_id` is accepted by default, and any
//!   non-empty `client_secret` passes. `--oauth2-clients id:secret,…` enables
//!   real validation so the `invalid_client` path is testable.
//!
//! Process-global state is deliberate: `create_app` runs once per actix
//! worker, and signing/verification must agree across workers (and across the
//! app instances tests build). `LazyLock` statics share one key and one cache
//! for the whole process; the clients table stays per-app in `AppConfig`.

use crate::AppConfig;
use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub mod authorize;
pub mod token;
pub mod userinfo;

pub use authorize::{oauth2_authorize_get_handler, oauth2_authorize_post_handler};
pub use token::oauth2_token_handler;
pub use userinfo::oauth2_userinfo_handler;

/// Authorization-code lifetime — RFC 6749 §4.1.2 RECOMMENDED maximum.
const CODE_TTL_SECS: i64 = 600;
/// Access-token lifetime; matches the `expires_in` we advertise (§5.1).
const ACCESS_TTL_SECS: i64 = 3600;
/// Refresh-token lifetime; refresh tokens are single-use (rotating).
const REFRESH_TTL_SECS: i64 = 14 * 24 * 3600;
/// Random 16-hex-char nonce for token uniqueness.
fn rand_jti() -> String {
    Uuid::new_v4().simple().to_string()[..16].to_string()
}

/// Per-process HMAC key (two UUIDv4s = 32 random bytes). Regenerating on
/// restart is the documented mock semantic, not a bug.
static KEY: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let mut key = Vec::with_capacity(32);
    key.extend_from_slice(Uuid::new_v4().as_bytes());
    key.extend_from_slice(Uuid::new_v4().as_bytes());
    key
});

/// Replay cache: fingerprint → evict-after instant. Fingerprints are the MAC
/// segment of a token, so the cache never holds token material.
static SPENT: LazyLock<parking_lot::Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashMap::new()));

pub(crate) fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// HMAC-SHA256 (RFC 2104), hand-rolled on `sha2` to avoid a new direct dep.
/// Correctness pinned by the RFC 4231 test vector in `tests.rs`.
pub(crate) fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut block = [0u8; 64];
    if key.len() > 64 {
        let digest = Sha256::digest(key);
        block[..32].copy_from_slice(&digest);
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let ipad: Vec<u8> = block.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = block.iter().map(|b| b ^ 0x5c).collect();
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    outer.finalize().to_vec()
}

/// Constant-time byte comparison for MAC checks.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Authorization code payload (short keys keep envelopes compact).
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct CodeClaims {
    pub cid: String,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub st: Option<String>,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chm: Option<String>,
    /// Per-code nonce: parallel consents with identical parameters in the
    /// same second must still yield distinct codes.
    #[serde(default)]
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

/// Access (`typ == "at"`) and refresh (`typ == "rt"`) token payload. The type
/// tag prevents a refresh token being replayed as an access token and vice
/// versa.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct BearerClaims {
    pub cid: String,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub typ: String,
    /// Per-token nonce: two tokens minted in the same second with identical
    /// claims must still differ, or refresh rotation would hand back a token
    /// the replay cache already considers spent.
    #[serde(default)]
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

impl CodeClaims {
    pub fn new(cid: String, uri: String, st: Option<String>, email: String) -> Self {
        let iat = now_unix();
        Self {
            cid,
            uri,
            st,
            email,
            scope: None,
            chal: None,
            chm: None,
            jti: rand_jti(),
            iat,
            exp: iat + CODE_TTL_SECS,
        }
    }
}

impl BearerClaims {
    pub fn access(cid: String, email: String, scope: Option<String>) -> Self {
        let iat = now_unix();
        Self {
            cid,
            email,
            scope,
            typ: "at".into(),
            jti: rand_jti(),
            iat,
            exp: iat + ACCESS_TTL_SECS,
        }
    }

    pub fn refresh(cid: String, email: String, scope: Option<String>) -> Self {
        let iat = now_unix();
        Self {
            cid,
            email,
            scope,
            typ: "rt".into(),
            jti: rand_jti(),
            iat,
            exp: iat + REFRESH_TTL_SECS,
        }
    }
}

/// Sign a claims payload into the `payload.mac` envelope.
pub(crate) fn sign<T: Serialize>(claims: &T) -> String {
    let payload = serde_json::to_vec(claims).expect("claims serialize");
    let mac = hmac_sha256(&KEY, &payload);
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(mac)
    )
}

/// Verify an envelope: MAC check (constant-time) then JSON parse.
pub(crate) fn verify<T: DeserializeOwned>(token: &str) -> Option<T> {
    let (payload_b64, mac_b64) = token.split_once('.')?;
    let payload = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let mac = URL_SAFE_NO_PAD.decode(mac_b64).ok()?;
    if !ct_eq(&mac, &hmac_sha256(&KEY, &payload)) {
        return None;
    }
    serde_json::from_slice(&payload).ok()
}

/// The MAC segment of a token — replay-cache key that avoids storing token
/// material.
fn fingerprint(token: &str) -> Option<String> {
    token.split_once('.').map(|(_, mac)| mac.to_string())
}

/// Atomically mark a one-time token as spent. Returns `false` — rejection —
/// when it was already spent. Prunes expired entries on the way in; the map
/// holds only live 10-minute codes and 14-day refresh fingerprints.
pub(crate) fn spend(token: &str, ttl: Duration) -> bool {
    let Some(fp) = fingerprint(token) else {
        return false;
    };
    let mut spent = SPENT.lock();
    let now = Instant::now();
    spent.retain(|_, until| *until > now);
    if spent.contains_key(&fp) {
        return false;
    }
    spent.insert(fp, now + ttl);
    true
}

/// Token-endpoint error: RFC 6749 §5.2 JSON shape. `no-store`/`no-cache` on
/// error responses too (§5.2 example); `invalid_client` carries the Basic
/// challenge (§5.2 allows 401; MUST when the client attempted Basic).
pub(crate) fn token_error(status: StatusCode, error: &str, description: &str) -> HttpResponse {
    let mut builder = HttpResponse::build(status);
    builder.insert_header(("Cache-Control", "no-store"));
    builder.insert_header(("Pragma", "no-cache"));
    if error == "invalid_client" {
        builder.insert_header(("WWW-Authenticate", "Basic"));
    }
    builder.json(json!({
        "error": error,
        "error_description": description,
    }))
}

/// Token-endpoint success: §5.1 shape with the §5.1-required cache headers.
pub(crate) fn token_success(
    access: &str,
    refresh: Option<&str>,
    scope: Option<&str>,
) -> HttpResponse {
    let mut body = json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": ACCESS_TTL_SECS,
    });
    if let Some(refresh) = refresh {
        body["refresh_token"] = json!(refresh);
    }
    if let Some(scope) = scope {
        body["scope"] = json!(scope);
    }
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .insert_header(("Pragma", "no-cache"))
        .json(body)
}

/// HTML-escape every dynamic field rendered into the consent and error pages.
/// There is no template layer here, so this is the only XSS defense.
pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Validate that a redirect target is an absolute http(s) URI. RFC 6749
/// §3.1.2.4: requests failing this must NOT be redirected.
pub(crate) fn is_http_url(uri: &str) -> bool {
    url::Url::parse(uri)
        .map(|u| u.scheme() == "http" || u.scheme() == "https")
        .unwrap_or(false)
}

/// 302 carrying OAuth parameters in the query component (code flow, §4.1.2)
/// or the fragment component (implicit flow, §4.2.2 / §4.2.2.1).
pub(crate) fn oauth_redirect(
    target: &str,
    params: &[(String, String)],
    fragment: bool,
) -> HttpResponse {
    let Ok(mut url) = url::Url::parse(target) else {
        return HttpResponse::BadRequest().body("Invalid redirect_uri");
    };
    if fragment {
        let qs = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        url.set_fragment(Some(&qs));
    } else {
        for (k, v) in params {
            url.query_pairs_mut().append_pair(k, v);
        }
    }
    HttpResponse::Found()
        .insert_header(("Location", url.as_str()))
        .finish()
}

/// State parameter echo: RFC 6749 sends `state` back only when the client
/// sent one (§4.1.2 "REQUIRED if present").
pub(crate) fn state_param(st: Option<&str>) -> Option<(String, String)> {
    st.filter(|s| !s.is_empty())
        .map(|s| ("state".to_string(), s.to_string()))
}

/// `GET /oauth2` — self-describing index (the `/llm` pattern): endpoints,
/// supported grants, PKCE methods, and the mock's state semantics.
pub async fn oauth2_index_handler() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "service": "httpcan OAuth 2.0 mock",
        "note": "Mock authorization server. Any credentials are accepted unless --oauth2-clients is configured; the consent email is the only identity. One-time codes and refresh rotation are enforced in-process: restarting the server invalidates everything issued.",
        "spec": "RFC 6749 (OAuth 2.0); PKCE per RFC 7636; discovery per RFC 8414",
        "deprecated_grants": "implicit and password are implemented for legacy-client compatibility only; both are removed in OAuth 2.1",
        "endpoints": {
            "authorize": "/oauth2/authorize",
            "token": "/oauth2/token",
            "userinfo": "/oauth2/userinfo",
            "discovery": "/.well-known/oauth-authorization-server"
        },
        "response_types_supported": ["code", "token"],
        "grant_types_supported": ["authorization_code", "implicit", "password", "refresh_token", "client_credentials"],
        "code_challenge_methods_supported": ["S256", "plain"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
    }))
}

/// `GET /.well-known/oauth-authorization-server` — RFC 8414 metadata with the
/// issuer resolved against the request origin (mirrors `api_catalog_handler`),
/// so discovery clients configure themselves against whatever host serves
/// them.
pub async fn oauth2_metadata_handler(req: HttpRequest) -> HttpResponse {
    let conn = req.connection_info();
    let base = format!("{}://{}", conn.scheme(), conn.host());
    HttpResponse::Ok().json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth2/authorize"),
        "token_endpoint": format!("{base}/oauth2/token"),
        "userinfo_endpoint": format!("{base}/oauth2/userinfo"),
        "service_documentation": format!("{base}/"),
        "response_types_supported": ["code", "token"],
        "grant_types_supported": ["authorization_code", "implicit", "password", "refresh_token", "client_credentials"],
        "code_challenge_methods_supported": ["S256", "plain"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
    }))
}

/// Placeholder identity for the client_credentials grant (§4.4): there is no
/// resource owner, so the mock mints a stable stand-in email.
pub(crate) const CLIENT_CREDENTIALS_EMAIL: &str = "client@httpcan.local";

/// Duration constants re-exported for `spend` callers.
pub(crate) fn code_ttl() -> Duration {
    Duration::from_secs(CODE_TTL_SECS as u64)
}

pub(crate) fn refresh_ttl() -> Duration {
    Duration::from_secs(REFRESH_TTL_SECS as u64)
}

/// Convenience accessor for the `web::Data<AppConfig>` clients table.
pub(crate) fn clients_table(config: &web::Data<AppConfig>) -> Option<&HashMap<String, String>> {
    config.oauth2_clients.as_ref()
}
