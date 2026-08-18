//! `GET/POST /oauth2/userinfo` — a Bearer-protected resource (RFC 6750
//! usage): verifies the access token and returns the mock identity. The 401
//! challenges carry `error="invalid_token"` attributes once a token was
//! presented but rejected (RFC 6750 §3), instead of a bare
//! `WWW-Authenticate: Bearer`.

use super::verify;
use actix_web::{HttpRequest, HttpResponse};
use serde_json::json;

pub async fn oauth2_userinfo_handler(req: HttpRequest) -> HttpResponse {
    let header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let Some(token) = header.strip_prefix("Bearer ").filter(|t| !t.is_empty()) else {
        return unauthorized(None);
    };

    let Some(claims) = verify::<super::BearerClaims>(token) else {
        return unauthorized(Some("Invalid access token"));
    };
    if claims.typ != "at" {
        return unauthorized(Some("Token is not an access token"));
    }
    if super::now_unix() >= claims.exp {
        return unauthorized(Some("Access token has expired"));
    }

    let mut body = json!({
        "sub": claims.email,
        "email": claims.email,
        "client_id": claims.cid,
    });
    if let Some(scope) = claims.scope {
        body["scope"] = json!(scope);
    }
    HttpResponse::Ok().json(body)
}

fn unauthorized(detail: Option<&str>) -> HttpResponse {
    let challenge = match detail {
        Some(detail) => format!("Bearer error=\"invalid_token\", error_description=\"{detail}\""),
        None => "Bearer".to_string(),
    };
    HttpResponse::Unauthorized()
        .insert_header(("WWW-Authenticate", challenge))
        .json(json!({
            "error": "invalid_token",
            "error_description": detail.unwrap_or("Missing Authorization header"),
        }))
}
