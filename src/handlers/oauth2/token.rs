//! `POST /oauth2/token` — the token endpoint (RFC 6749 §3.2) with four
//! grants: `authorization_code` (+PKCE), `password`, `refresh_token`
//! (rotating), and `client_credentials`. The implicit grant needs no token
//! endpoint round-trip — it is served entirely at the authorize endpoint.
//!
//! Client authentication (§2.3.1/§3.2.1) accepts both channels:
//! `Authorization: Basic` (client_secret_basic) and form-body
//! client_id/client_secret (client_secret_post). Using both at once is
//! rejected (§5.2 `invalid_request` "multiple credentials"). Without
//! `--oauth2-clients` configured the mock accepts any non-empty secret.

use super::{
    clients_table, code_ttl, refresh_ttl, sign, spend, token_error, token_success, verify,
    BearerClaims, CodeClaims, CLIENT_CREDENTIALS_EMAIL,
};
use actix_web::web::{Data, Form};
use actix_web::{HttpRequest, HttpResponse};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
/// `POST /oauth2/token`.
pub async fn oauth2_token_handler(
    req: HttpRequest,
    config: Data<crate::AppConfig>,
    form: Form<HashMap<String, String>>,
) -> HttpResponse {
    // Client authentication, both RFC channels.
    let basic = basic_credentials(&req);
    let form_id = form.get("client_id").cloned().filter(|s| !s.is_empty());
    let form_secret = form.get("client_secret").cloned().filter(|s| !s.is_empty());

    let (client_id, client_secret) = match (basic, form_id) {
        (Some((bid, bsecret)), Some(fid)) => {
            if bid != fid {
                return token_error(
                    actix_web::http::StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "Multiple client credentials (Basic and body disagree)",
                );
            }
            (bid, Some(bsecret))
        }
        (Some((bid, bsecret)), None) => (bid, Some(bsecret)),
        (None, Some(fid)) => (fid, form_secret),
        (None, None) => {
            return token_error(
                actix_web::http::StatusCode::BAD_REQUEST,
                "invalid_request",
                "Missing required parameter: client_id",
            );
        }
    };

    // Real validation when a clients table is configured; otherwise the mock
    // accepts any credentials.
    if let Some(clients) = clients_table(&config) {
        match clients.get(&client_id) {
            Some(expected) if Some(expected) == client_secret.as_ref() => {}
            _ => {
                return token_error(
                    actix_web::http::StatusCode::UNAUTHORIZED,
                    "invalid_client",
                    "Client authentication failed",
                );
            }
        }
    }

    let grant = form.get("grant_type").map(String::as_str).unwrap_or("");
    match grant {
        "authorization_code" => grant_authorization_code(&client_id, &form),
        "password" => grant_password(&client_id, &form),
        "refresh_token" => grant_refresh_token(&client_id, &form),
        "client_credentials" => grant_client_credentials(&client_id, &form),
        "" => token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing required parameter: grant_type",
        ),
        _ => token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "Supported: authorization_code, password, refresh_token, client_credentials",
        ),
    }
}

/// Decode `Authorization: Basic` per RFC 6749 §2.3.1: split on the first
/// colon, percent-decode both halves (the crate only decodes %XX — a literal
/// `+` survives, which is the lenient behavior real-world clients need).
/// Structurally malformed headers are treated as absent credentials.
fn basic_credentials(req: &HttpRequest) -> Option<(String, String)> {
    let value = req.headers().get("Authorization")?.to_str().ok()?;
    let encoded = value.strip_prefix("Basic ")?;
    let decoded = STANDARD.decode(encoded.trim()).ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let (id, secret) = text.split_once(':')?;
    Some((
        urlencoding::decode(id)
            .map(Into::into)
            .unwrap_or_else(|_| id.to_string()),
        urlencoding::decode(secret)
            .map(Into::into)
            .unwrap_or_else(|_| secret.to_string()),
    ))
}

/// `grant_type=authorization_code` (§4.1.3): verify signature and expiry,
/// the client_id and redirect_uri bindings, PKCE, then enforce one-time use
/// atomically (§4.1.2 MUST).
fn grant_authorization_code(client_id: &str, form: &HashMap<String, String>) -> HttpResponse {
    let bad = |desc: &str| {
        token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_grant",
            desc,
        )
    };
    let Some(code) = form.get("code").filter(|s| !s.is_empty()) else {
        return token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing required parameter: code",
        );
    };
    let Some(redirect_uri) = form.get("redirect_uri").filter(|s| !s.is_empty()) else {
        return token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing required parameter: redirect_uri",
        );
    };

    let Some(claims) = verify::<CodeClaims>(code) else {
        return bad("Invalid authorization code");
    };
    if super::now_unix() >= claims.exp {
        return bad("Authorization code has expired");
    }
    if claims.cid != client_id {
        return bad("client_id does not match the authorization request");
    }
    if claims.uri != redirect_uri.as_str() {
        return bad("redirect_uri does not match the authorization request");
    }

    // PKCE (RFC 7636 §4.6): only when the authorize request carried a
    // challenge. Missing verifier → invalid_request; wrong verifier →
    // invalid_grant.
    if let Some(challenge) = claims.chal.as_deref() {
        let Some(verifier) = form.get("code_verifier").filter(|s| !s.is_empty()) else {
            return token_error(
                actix_web::http::StatusCode::BAD_REQUEST,
                "invalid_request",
                "Missing required parameter: code_verifier",
            );
        };
        let ok = match claims.chm.as_deref() {
            Some("S256") => {
                let digest = Sha256::digest(verifier.as_bytes());
                let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
                encoded == challenge
            }
            _ => verifier == challenge, // "plain"
        };
        if !ok {
            return bad("PKCE code_verifier does not match the challenge");
        }
    }

    // One-time use — check and mark under the replay cache's lock.
    if !spend(code, code_ttl()) {
        return bad("Authorization code has already been redeemed");
    }

    let access = BearerClaims::access(
        claims.cid.clone(),
        claims.email.clone(),
        claims.scope.clone(),
    );
    let refresh = BearerClaims::refresh(
        claims.cid.clone(),
        claims.email.clone(),
        claims.scope.clone(),
    );
    token_success(
        &sign(&access),
        Some(&sign(&refresh)),
        claims.scope.as_deref(),
    )
}

/// `grant_type=password` (§4.3, legacy compatibility): any credentials are
/// accepted — the username doubles as the mock identity.
fn grant_password(client_id: &str, form: &HashMap<String, String>) -> HttpResponse {
    let Some(username) = form.get("username").filter(|s| !s.is_empty()) else {
        return token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing required parameter: username",
        );
    };
    if form.get("password").filter(|s| !s.is_empty()).is_none() {
        return token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing required parameter: password",
        );
    }
    let scope = form.get("scope").cloned().filter(|s| !s.is_empty());
    let access = BearerClaims::access(client_id.to_string(), username.clone(), scope.clone());
    let refresh = BearerClaims::refresh(client_id.to_string(), username.clone(), scope.clone());
    token_success(&sign(&access), Some(&sign(&refresh)), scope.as_deref())
}

/// `grant_type=refresh_token` (§6): verifies type tag, expiry and client
/// binding, then rotates — the presented refresh token is spent and a fresh
/// access/refresh pair is issued.
fn grant_refresh_token(client_id: &str, form: &HashMap<String, String>) -> HttpResponse {
    let bad = |desc: &str| {
        token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_grant",
            desc,
        )
    };
    let Some(token) = form.get("refresh_token").filter(|s| !s.is_empty()) else {
        return token_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "invalid_request",
            "Missing required parameter: refresh_token",
        );
    };
    let Some(claims) = verify::<BearerClaims>(token) else {
        return bad("Invalid refresh token");
    };
    if claims.typ != "rt" {
        return bad("Token is not a refresh token");
    }
    if super::now_unix() >= claims.exp {
        return bad("Refresh token has expired");
    }
    if claims.cid != client_id {
        return bad("Refresh token was issued to another client");
    }
    if !spend(token, refresh_ttl()) {
        return bad("Refresh token has already been rotated");
    }

    let access = BearerClaims::access(
        claims.cid.clone(),
        claims.email.clone(),
        claims.scope.clone(),
    );
    let refresh = BearerClaims::refresh(claims.cid, claims.email, claims.scope.clone());
    token_success(
        &sign(&access),
        Some(&sign(&refresh)),
        claims.scope.as_deref(),
    )
}

/// `grant_type=client_credentials` (§4.4): no resource owner, so the mock
/// mints a stand-in identity. §4.4.3: no refresh token is issued.
fn grant_client_credentials(client_id: &str, form: &HashMap<String, String>) -> HttpResponse {
    let scope = form.get("scope").cloned().filter(|s| !s.is_empty());
    let access = BearerClaims::access(
        client_id.to_string(),
        CLIENT_CREDENTIALS_EMAIL.to_string(),
        scope.clone(),
    );
    token_success(&sign(&access), None, scope.as_deref())
}
