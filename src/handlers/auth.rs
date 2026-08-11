use super::*;
use chrono::DateTime;
use jsonwebtoken::{dangerous::insecure_decode, decode_header};
use md5;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512, Sha512_256};
use std::collections::HashMap;

// Function to parse digest authentication header
fn parse_digest_auth(auth_header: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    if !auth_header.starts_with("Digest ") {
        return params;
    }

    let auth_params = &auth_header[7..]; // Remove "Digest " prefix

    for param in auth_params.split(',') {
        let param = param.trim();
        if let Some(eq_pos) = param.find('=') {
            let key = param[..eq_pos].trim().to_string();
            let value = param[eq_pos + 1..].trim();
            // Remove quotes if present
            let value = if value.starts_with('"') && value.ends_with('"') {
                value[1..value.len() - 1].to_string()
            } else {
                value.to_string()
            };
            params.insert(key, value);
        }
    }

    params
}

// Parameters for digest authentication calculation
#[derive(Debug)]
struct DigestParams<'a> {
    username: &'a str,
    password: &'a str,
    realm: &'a str,
    method: &'a str,
    uri: &'a str,
    nonce: &'a str,
    algorithm: &'a str,
    qop: Option<&'a str>,
    nc: Option<&'a str>,
    cnonce: Option<&'a str>,
    body: Option<&'a [u8]>,
}

/// Lowercase hex encoding of a byte slice.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

/// Hex digest for the given algorithm (MD5/SHA-256/SHA-512/SHA-512-256), or None.
pub(crate) fn digest_hash_hex(algorithm: &str, data: &[u8]) -> Option<String> {
    Some(match algorithm {
        "MD5" => format!("{:x}", md5::compute(data)),
        "SHA-256" => {
            let mut h = Sha256::new();
            h.update(data);
            hex_lower(h.finalize().as_slice())
        }
        "SHA-512" => {
            let mut h = Sha512::new();
            h.update(data);
            hex_lower(h.finalize().as_slice())
        }
        "SHA-512-256" => {
            let mut h = Sha512_256::new();
            h.update(data);
            hex_lower(h.finalize().as_slice())
        }
        _ => return None,
    })
}

// Function to calculate digest hash with QOP support
fn calculate_digest_response(params: DigestParams) -> String {
    let DigestParams {
        username,
        password,
        realm,
        method,
        uri,
        nonce,
        algorithm,
        qop,
        nc,
        cnonce,
        body,
    } = params;

    // HA1 = H(username:realm:password)
    let ha1 = match digest_hash_hex(
        algorithm,
        format!("{username}:{realm}:{password}").as_bytes(),
    ) {
        Some(v) => v,
        None => return String::new(),
    };

    // HA2: auth-int = H(method:uri:H(body)), otherwise H(method:uri)
    let ha2 = match qop {
        Some("auth-int") => {
            let body_hash = match digest_hash_hex(algorithm, body.unwrap_or(&[])) {
                Some(v) => v,
                None => return String::new(),
            };
            match digest_hash_hex(algorithm, format!("{method}:{uri}:{body_hash}").as_bytes()) {
                Some(v) => v,
                None => return String::new(),
            }
        }
        _ => match digest_hash_hex(algorithm, format!("{method}:{uri}").as_bytes()) {
            Some(v) => v,
            None => return String::new(),
        },
    };

    // Response: with qop = H(HA1:nonce:nc:cnonce:qop:HA2), else H(HA1:nonce:HA2)
    let response_input = match qop {
        Some("auth") | Some("auth-int") => {
            if let (Some(nc), Some(cnonce)) = (nc, cnonce) {
                format!("{ha1}:{nonce}:{nc}:{cnonce}:{}:{ha2}", qop.unwrap())
            } else {
                return String::new();
            }
        }
        _ => format!("{ha1}:{nonce}:{ha2}"),
    };

    digest_hash_hex(algorithm, response_input.as_bytes()).unwrap_or_default()
}

// Function to calculate next stale_after value
fn next_stale_after_value(current: &str) -> String {
    match current {
        "never" => "never".to_string(),
        _ => {
            if let Ok(num) = current.parse::<i32>() {
                if num > 0 {
                    (num - 1).to_string()
                } else {
                    "0".to_string()
                }
            } else {
                "0".to_string()
            }
        }
    }
}

// Function to check if require-cookie parameter is enabled
fn is_require_cookie_enabled(req: &HttpRequest) -> bool {
    if let Some(query_string) = req
        .query_string()
        .split('&')
        .find(|param| param.starts_with("require-cookie"))
    {
        if let Some(value) = query_string.split('=').nth(1) {
            matches!(value.to_lowercase().as_str(), "1" | "t" | "true")
        } else {
            false
        }
    } else {
        false
    }
}

// Function to generate digest challenge response.
// `qop = None` omits the qop directive entirely (RFC 2069 legacy mode,
// httpbin #592); `Some(q)` advertises that qop value.
fn generate_digest_challenge(
    host: &str,
    qop: Option<&str>,
    algorithm: &str,
    stale: bool,
) -> String {
    let nonce = format!("{:x}", rand::random::<u64>());
    let opaque = format!("{:x}", rand::random::<u64>());

    let mut challenge = format!(
        "Digest realm=\"httpcan@{}\", nonce=\"{}\", opaque=\"{}\"",
        host, nonce, opaque
    );

    if let Some(q) = qop {
        challenge.push_str(&format!(", qop=\"{}\"", q));
    }

    // Always include algorithm for clarity, even for MD5
    challenge.push_str(&format!(", algorithm=\"{}\"", algorithm));

    if stale {
        challenge.push_str(", stale=TRUE");
    }

    challenge
}

// Resolve the URL qop path segment to the effective qop used for both the
// challenge and response validation:
// - "none"            => omit qop (RFC 2069 legacy mode, httpbin #592)
// - "auth"/"auth-int" => that value
// - anything else     => default to "auth"
fn resolve_qop(qop_param: &str) -> Option<&'static str> {
    match qop_param {
        "none" => None,
        "auth" => Some("auth"),
        "auth-int" => Some("auth-int"),
        _ => Some("auth"),
    }
}

pub async fn basic_auth_handler(
    _req: HttpRequest,
    path: web::Path<(String, String)>,
    auth: Option<BasicAuth>,
) -> Result<HttpResponse> {
    let (expected_user, expected_passwd) = path.into_inner();

    match auth {
        Some(auth) => {
            let user = auth.user_id();
            let password = auth.password().unwrap_or("");

            if user == expected_user && password == expected_passwd {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": user
                })))
            } else {
                Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", "Basic realm=\"Fake Realm\""))
                    .json(json!({
                        "authenticated": false
                    })))
            }
        }
        None => Ok(HttpResponse::Unauthorized()
            .append_header(("WWW-Authenticate", "Basic realm=\"Fake Realm\""))
            .json(json!({
                "authenticated": false
            }))),
    }
}

pub async fn basic_auth_user_only_handler(
    _req: HttpRequest,
    path: web::Path<String>,
    auth: Option<BasicAuth>,
) -> Result<HttpResponse> {
    let expected_user = path.into_inner();

    match auth {
        Some(auth) => {
            let user = auth.user_id();
            let password = auth.password().unwrap_or("");

            if user == expected_user && password.is_empty() {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": user
                })))
            } else {
                Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", "Basic realm=\"Fake Realm\""))
                    .json(json!({
                        "authenticated": false
                    })))
            }
        }
        None => Ok(HttpResponse::Unauthorized()
            .append_header(("WWW-Authenticate", "Basic realm=\"Fake Realm\""))
            .json(json!({
                "authenticated": false
            }))),
    }
}

pub async fn hidden_basic_auth_handler(
    _req: HttpRequest,
    path: web::Path<(String, String)>,
    auth: Option<BasicAuth>,
) -> Result<HttpResponse> {
    let (expected_user, expected_passwd) = path.into_inner();

    match auth {
        Some(auth) => {
            let user = auth.user_id();
            let password = auth.password().unwrap_or("");

            if user == expected_user && password == expected_passwd {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": user
                })))
            } else {
                Ok(HttpResponse::NotFound().json(json!({
                    "authenticated": false
                })))
            }
        }
        None => Ok(HttpResponse::NotFound().json(json!({
            "authenticated": false
        }))),
    }
}

pub async fn hidden_basic_auth_user_only_handler(
    _req: HttpRequest,
    path: web::Path<String>,
    auth: Option<BasicAuth>,
) -> Result<HttpResponse> {
    let expected_user = path.into_inner();

    match auth {
        Some(auth) => {
            let user = auth.user_id();
            let password = auth.password().unwrap_or("");

            if user == expected_user && password.is_empty() {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": user
                })))
            } else {
                Ok(HttpResponse::NotFound().json(json!({
                    "authenticated": false
                })))
            }
        }
        None => Ok(HttpResponse::NotFound().json(json!({
            "authenticated": false
        }))),
    }
}

pub async fn bearer_auth_handler(
    _req: HttpRequest,
    auth: Option<BearerAuth>,
) -> Result<HttpResponse> {
    match auth {
        Some(auth) => Ok(HttpResponse::Ok().json(json!({
            "authenticated": true,
            "token": auth.token()
        }))),
        None => Ok(HttpResponse::Unauthorized()
            .append_header(("WWW-Authenticate", "Bearer"))
            .json(json!({
                "authenticated": false
            }))),
    }
}

pub async fn digest_auth_handler(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (qop_param, expected_user, expected_passwd) = path.into_inner();

    // Check if require-cookie is enabled
    let require_cookie = is_require_cookie_enabled(&req);

    let auth_header = req.headers().get("Authorization");

    match auth_header {
        Some(auth_header_value) => {
            let auth_str = auth_header_value.to_str().unwrap_or("");
            let digest_params = parse_digest_auth(auth_str);

            // Check cookie requirement if enabled
            if require_cookie && req.headers().get("Cookie").is_none() {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    "MD5",
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": "Cookie header required but missing"
                    })));
            }

            if require_cookie {
                if let Some(cookies) = req.headers().get("Cookie") {
                    let cookie_str = cookies.to_str().unwrap_or("");
                    if !cookie_str.contains("fake=fake_value") {
                        return Ok(HttpResponse::Forbidden().json(json!({
                            "authenticated": false,
                            "error": "Missing cookie set on challenge",
                            "errors": ["missing cookie set on challenge"]
                        })));
                    }
                } else {
                    let challenge = generate_digest_challenge(
                        req.connection_info().host(),
                        resolve_qop(&qop_param),
                        "MD5",
                        false,
                    );

                    return Ok(HttpResponse::Unauthorized()
                        .append_header(("WWW-Authenticate", challenge.as_str()))
                        .json(json!({
                            "authenticated": false,
                            "error": "Cookie header required but missing"
                        })));
                }
            }

            // Extract required parameters
            let username = digest_params.get("username").cloned().unwrap_or_default();
            let realm = digest_params.get("realm").cloned().unwrap_or_default();
            let nonce = digest_params.get("nonce").cloned().unwrap_or_default();
            let uri = digest_params.get("uri").cloned().unwrap_or_default();
            let response = digest_params.get("response").cloned().unwrap_or_default();
            let algorithm = digest_params
                .get("algorithm")
                .cloned()
                .unwrap_or("MD5".to_string());
            let qop = digest_params.get("qop").cloned();
            let nc = digest_params.get("nc").cloned();
            let cnonce = digest_params.get("cnonce").cloned();

            // Verify username matches expected
            if username != expected_user {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    "MD5",
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": format!("Username mismatch. Expected '{}' but got '{}'", expected_user, username)
                    })));
            }

            // Validate QOP parameter
            let effective_qop = resolve_qop(&qop_param);

            // Verify QOP consistency
            if let Some(expected_qop) = effective_qop {
                if let Some(provided_qop) = &qop {
                    if provided_qop != expected_qop {
                        let challenge = generate_digest_challenge(
                            req.connection_info().host(),
                            resolve_qop(&qop_param),
                            &algorithm,
                            false,
                        );

                        return Ok(HttpResponse::Unauthorized()
                            .append_header(("WWW-Authenticate", challenge.as_str()))
                            .json(json!({
                                "authenticated": false,
                                "error": format!("QOP mismatch. Expected '{}' but got '{}'", expected_qop, provided_qop)
                            })));
                    }
                } else if expected_qop != "auth" && expected_qop != "auth-int" {
                    let challenge = generate_digest_challenge(
                        req.connection_info().host(),
                        resolve_qop(&qop_param),
                        &algorithm,
                        false,
                    );

                    return Ok(HttpResponse::Unauthorized()
                        .append_header(("WWW-Authenticate", challenge.as_str()))
                        .json(json!({
                            "authenticated": false,
                            "error": "QOP parameter required but missing from Authorization header"
                        })));
                }
            }

            // Calculate expected response
            let method = req.method().as_str();
            let expected_response = calculate_digest_response(DigestParams {
                username: &expected_user,
                password: &expected_passwd,
                realm: &realm,
                method,
                uri: &uri,
                nonce: &nonce,
                algorithm: &algorithm,
                qop: effective_qop,
                nc: nc.as_deref(),
                cnonce: cnonce.as_deref(),
                body: Some(&body),
            });

            // Verify the response hash
            if response == expected_response {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": expected_user
                })))
            } else {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    "MD5",
                    false,
                );

                Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": "Invalid credentials"
                    })))
            }
        }
        None => {
            // Generate challenge response
            let qop_value = resolve_qop(&qop_param);

            let challenge =
                generate_digest_challenge(req.connection_info().host(), qop_value, "MD5", false);

            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", challenge.as_str()))
                .json(json!({
                    "authenticated": false
                })))
        }
    }
}

pub async fn digest_auth_with_algorithm_handler(
    req: HttpRequest,
    path: web::Path<(String, String, String, String)>,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (qop_param, expected_user, expected_passwd, algorithm) = path.into_inner();

    // Validate algorithm parameter - only accept MD5, SHA-256, SHA-512
    if !matches!(
        algorithm.as_str(),
        "MD5" | "SHA-256" | "SHA-512" | "SHA-512-256"
    ) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid algorithm. Supported algorithms: MD5, SHA-256, SHA-512, SHA-512-256"
        })));
    }

    // Check if require-cookie is enabled
    let require_cookie = is_require_cookie_enabled(&req);

    let auth_header = req.headers().get("Authorization");

    match auth_header {
        Some(auth_header_value) => {
            let auth_str = auth_header_value.to_str().unwrap_or("");
            let digest_params = parse_digest_auth(auth_str);

            // Check cookie requirement if enabled
            if require_cookie && req.headers().get("Cookie").is_none() {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": "Cookie header required but missing"
                    })));
            }

            if require_cookie {
                if let Some(cookies) = req.headers().get("Cookie") {
                    let cookie_str = cookies.to_str().unwrap_or("");
                    if !cookie_str.contains("fake=fake_value") {
                        return Ok(HttpResponse::Forbidden().json(json!({
                            "authenticated": false,
                            "error": "Missing cookie set on challenge",
                            "errors": ["missing cookie set on challenge"]
                        })));
                    }
                } else {
                    let challenge = generate_digest_challenge(
                        req.connection_info().host(),
                        resolve_qop(&qop_param),
                        &algorithm,
                        false,
                    );

                    return Ok(HttpResponse::Unauthorized()
                        .append_header(("WWW-Authenticate", challenge.as_str()))
                        .json(json!({
                            "authenticated": false,
                            "error": "Cookie header required but missing"
                        })));
                }
            }

            // Extract required parameters
            let username = digest_params.get("username").cloned().unwrap_or_default();
            let realm = digest_params.get("realm").cloned().unwrap_or_default();
            let nonce = digest_params.get("nonce").cloned().unwrap_or_default();
            let uri = digest_params.get("uri").cloned().unwrap_or_default();
            let response = digest_params.get("response").cloned().unwrap_or_default();
            let auth_algorithm = digest_params
                .get("algorithm")
                .cloned()
                .unwrap_or("MD5".to_string());
            let qop = digest_params.get("qop").cloned();
            let nc = digest_params.get("nc").cloned();
            let cnonce = digest_params.get("cnonce").cloned();

            // Check if algorithm in Authorization header matches URL parameter
            if auth_algorithm != algorithm {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": format!("Algorithm mismatch. URL specifies '{}' but Authorization header uses '{}'", algorithm, auth_algorithm)
                    })));
            }

            // Verify username matches expected
            if username != expected_user {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": format!("Username mismatch. Expected '{}' but got '{}'", expected_user, username)
                    })));
            }

            // Validate QOP parameter
            let effective_qop = resolve_qop(&qop_param);

            // Verify QOP consistency
            if let Some(expected_qop) = effective_qop {
                if let Some(provided_qop) = &qop {
                    if provided_qop != expected_qop {
                        let challenge = generate_digest_challenge(
                            req.connection_info().host(),
                            resolve_qop(&qop_param),
                            &algorithm,
                            false,
                        );

                        return Ok(HttpResponse::Unauthorized()
                            .append_header(("WWW-Authenticate", challenge.as_str()))
                            .json(json!({
                                "authenticated": false,
                                "error": format!("QOP mismatch. Expected '{}' but got '{}'", expected_qop, provided_qop)
                            })));
                    }
                } else if expected_qop != "auth" && expected_qop != "auth-int" {
                    let challenge = generate_digest_challenge(
                        req.connection_info().host(),
                        resolve_qop(&qop_param),
                        &algorithm,
                        false,
                    );

                    return Ok(HttpResponse::Unauthorized()
                        .append_header(("WWW-Authenticate", challenge.as_str()))
                        .json(json!({
                            "authenticated": false,
                            "error": "QOP parameter required but missing from Authorization header"
                        })));
                }
            }

            // Calculate expected response
            let method = req.method().as_str();
            let expected_response = calculate_digest_response(DigestParams {
                username: &expected_user,
                password: &expected_passwd,
                realm: &realm,
                method,
                uri: &uri,
                nonce: &nonce,
                algorithm: &algorithm,
                qop: effective_qop,
                nc: nc.as_deref(),
                cnonce: cnonce.as_deref(),
                body: Some(&body),
            });

            // Verify the response hash
            if response == expected_response {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": expected_user,
                    "algorithm": algorithm
                })))
            } else {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": "Invalid credentials"
                    })))
            }
        }
        None => {
            // Generate challenge response
            let qop_value = resolve_qop(&qop_param);

            let challenge = generate_digest_challenge(
                req.connection_info().host(),
                qop_value,
                &algorithm,
                false,
            );

            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", challenge.as_str()))
                .json(json!({
                    "authenticated": false
                })))
        }
    }
}

pub async fn digest_auth_full_handler(
    req: HttpRequest,
    path: web::Path<(String, String, String, String, String)>,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let (qop_param, expected_user, expected_passwd, algorithm, stale_after) = path.into_inner();

    // Validate algorithm parameter - only accept MD5, SHA-256, SHA-512
    if !matches!(
        algorithm.as_str(),
        "MD5" | "SHA-256" | "SHA-512" | "SHA-512-256"
    ) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid algorithm. Supported algorithms: MD5, SHA-256, SHA-512, SHA-512-256"
        })));
    }

    // Check if require-cookie is enabled
    let require_cookie = is_require_cookie_enabled(&req);

    let auth_header = req.headers().get("Authorization");

    match auth_header {
        Some(auth_header_value) => {
            let auth_str = auth_header_value.to_str().unwrap_or("");
            let digest_params = parse_digest_auth(auth_str);

            // Check cookie requirement if enabled
            if require_cookie && req.headers().get("Cookie").is_none() {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .cookie(actix_web::cookie::Cookie::new("stale_after", &stale_after))
                    .cookie(actix_web::cookie::Cookie::new("fake", "fake_value"))
                    .json(json!({
                        "authenticated": false,
                        "error": "Cookie header required but missing"
                    })));
            }

            if require_cookie {
                if let Some(cookies) = req.headers().get("Cookie") {
                    let cookie_str = cookies.to_str().unwrap_or("");
                    if !cookie_str.contains("fake=fake_value") {
                        return Ok(HttpResponse::Forbidden()
                            .cookie(actix_web::cookie::Cookie::new("fake", "fake_value"))
                            .json(json!({
                                "authenticated": false,
                                "error": "Missing cookie set on challenge",
                                "errors": ["missing cookie set on challenge"]
                            })));
                    }
                } else {
                    let challenge = generate_digest_challenge(
                        req.connection_info().host(),
                        resolve_qop(&qop_param),
                        &algorithm,
                        false,
                    );

                    return Ok(HttpResponse::Unauthorized()
                        .append_header(("WWW-Authenticate", challenge.as_str()))
                        .cookie(actix_web::cookie::Cookie::new("stale_after", &stale_after))
                        .cookie(actix_web::cookie::Cookie::new("fake", "fake_value"))
                        .json(json!({
                            "authenticated": false,
                            "error": "Cookie header required but missing"
                        })));
                }
            }

            // Extract required parameters
            let username = digest_params.get("username").cloned().unwrap_or_default();
            let realm = digest_params.get("realm").cloned().unwrap_or_default();
            let nonce = digest_params.get("nonce").cloned().unwrap_or_default();
            let uri = digest_params.get("uri").cloned().unwrap_or_default();
            let response = digest_params.get("response").cloned().unwrap_or_default();
            let auth_algorithm = digest_params
                .get("algorithm")
                .cloned()
                .unwrap_or("MD5".to_string());
            let qop = digest_params.get("qop").cloned();
            let nc = digest_params.get("nc").cloned();
            let cnonce = digest_params.get("cnonce").cloned();

            // Check if algorithm in Authorization header matches URL parameter
            if auth_algorithm != algorithm {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": format!("Algorithm mismatch. URL specifies '{}' but Authorization header uses '{}'", algorithm, auth_algorithm)
                    })));
            }

            // Verify username matches expected
            if username != expected_user {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .json(json!({
                        "authenticated": false,
                        "error": format!("Username mismatch. Expected '{}' but got '{}'", expected_user, username)
                    })));
            }

            // Get stale_after value from cookies
            let mut stale_after_value = None;
            let mut last_nonce = None;

            if let Some(cookies) = req.headers().get("Cookie") {
                let cookie_str = cookies.to_str().unwrap_or("");
                for cookie_pair in cookie_str.split(';') {
                    let cookie_pair = cookie_pair.trim();
                    if let Some(eq_pos) = cookie_pair.find('=') {
                        let key = cookie_pair[..eq_pos].trim();
                        let value = cookie_pair[eq_pos + 1..].trim();
                        match key {
                            "stale_after" => stale_after_value = Some(value.to_string()),
                            "last_nonce" => last_nonce = Some(value.to_string()),
                            _ => {}
                        }
                    }
                }
            }

            // Check for stale nonce conditions
            let is_stale = if let Some(ref last_nonce_value) = last_nonce {
                nonce == *last_nonce_value
            } else {
                false
            };

            let is_stale_by_count = if let Some(ref stale_after_val) = stale_after_value {
                stale_after_val == "0"
            } else {
                false
            };

            if is_stale || is_stale_by_count {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    true, // stale=TRUE
                );

                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .cookie(actix_web::cookie::Cookie::new("stale_after", &stale_after))
                    .cookie(actix_web::cookie::Cookie::new("last_nonce", &nonce))
                    .cookie(actix_web::cookie::Cookie::new("fake", "fake_value"))
                    .json(json!({
                        "authenticated": false,
                        "error": "Stale nonce"
                    })));
            }

            // Validate QOP parameter
            let effective_qop = resolve_qop(&qop_param);

            // Verify QOP consistency
            if let Some(expected_qop) = effective_qop {
                if let Some(provided_qop) = &qop {
                    if provided_qop != expected_qop {
                        let challenge = generate_digest_challenge(
                            req.connection_info().host(),
                            resolve_qop(&qop_param),
                            &algorithm,
                            false,
                        );

                        return Ok(HttpResponse::Unauthorized()
                            .append_header(("WWW-Authenticate", challenge.as_str()))
                            .json(json!({
                                "authenticated": false,
                                "error": format!("QOP mismatch. Expected '{}' but got '{}'", expected_qop, provided_qop)
                            })));
                    }
                } else if expected_qop != "auth" && expected_qop != "auth-int" {
                    let challenge = generate_digest_challenge(
                        req.connection_info().host(),
                        resolve_qop(&qop_param),
                        &algorithm,
                        false,
                    );

                    return Ok(HttpResponse::Unauthorized()
                        .append_header(("WWW-Authenticate", challenge.as_str()))
                        .json(json!({
                            "authenticated": false,
                            "error": "QOP parameter required but missing from Authorization header"
                        })));
                }
            }

            // Calculate expected response
            let method = req.method().as_str();
            let expected_response = calculate_digest_response(DigestParams {
                username: &expected_user,
                password: &expected_passwd,
                realm: &realm,
                method,
                uri: &uri,
                nonce: &nonce,
                algorithm: &algorithm,
                qop: effective_qop,
                nc: nc.as_deref(),
                cnonce: cnonce.as_deref(),
                body: Some(&body),
            });

            // Verify the response hash
            if response == expected_response {
                let mut response_builder = HttpResponse::Ok();
                response_builder.cookie(actix_web::cookie::Cookie::new("fake", "fake_value"));

                // Update stale_after counter
                if let Some(stale_after_val) = stale_after_value {
                    let next_value = next_stale_after_value(&stale_after_val);
                    response_builder
                        .cookie(actix_web::cookie::Cookie::new("stale_after", next_value));
                }

                Ok(response_builder.json(json!({
                    "authenticated": true,
                    "user": expected_user
                })))
            } else {
                let challenge = generate_digest_challenge(
                    req.connection_info().host(),
                    resolve_qop(&qop_param),
                    &algorithm,
                    false,
                );

                Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", challenge.as_str()))
                    .cookie(actix_web::cookie::Cookie::new("stale_after", &stale_after))
                    .cookie(actix_web::cookie::Cookie::new("last_nonce", &nonce))
                    .cookie(actix_web::cookie::Cookie::new("fake", "fake_value"))
                    .json(json!({
                        "authenticated": false,
                        "error": "Invalid credentials"
                    })))
            }
        }
        None => {
            let challenge = generate_digest_challenge(
                req.connection_info().host(),
                resolve_qop(&qop_param),
                &algorithm,
                false,
            );

            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", challenge.as_str()))
                .cookie(actix_web::cookie::Cookie::new("stale_after", &stale_after))
                .cookie(actix_web::cookie::Cookie::new("fake", "fake_value"))
                .json(json!({
                    "authenticated": false
                })))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    #[serde(flatten)]
    standard_claims: HashMap<String, serde_json::Value>,
}

fn format_unix_timestamp(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| "Invalid timestamp".to_string())
}

fn validate_jwt_structure(token: &str) -> Result<(serde_json::Value, serde_json::Value), String> {
    // Split token into parts
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT structure: must have 3 parts separated by dots".to_string());
    }

    // Decode header
    let header = match decode_header(token) {
        Ok(h) => h,
        Err(e) => return Err(format!("Invalid JWT header: {}", e)),
    };

    // Decode payload without signature/exp validation (structure check only).
    let payload = match insecure_decode::<Claims>(token) {
        Ok(token_data) => token_data.claims.standard_claims,
        Err(e) => return Err(format!("Invalid JWT payload: {}", e)),
    };

    Ok((
        serde_json::to_value(header).unwrap(),
        serde_json::to_value(payload).unwrap(),
    ))
}

fn validate_jwt_expiration(payload: &serde_json::Value) -> (String, Option<i64>) {
    if let Some(exp_value) = payload.get("exp") {
        if let Some(exp) = exp_value.as_i64() {
            let now = chrono::Utc::now().timestamp();
            if exp < now {
                return ("expired".to_string(), Some(exp));
            } else {
                return ("valid".to_string(), Some(exp));
            }
        } else if let Some(exp) = exp_value.as_f64() {
            let exp = exp as i64;
            let now = chrono::Utc::now().timestamp();
            if exp < now {
                return ("expired".to_string(), Some(exp));
            } else {
                return ("valid".to_string(), Some(exp));
            }
        } else {
            return ("invalid_format".to_string(), None);
        }
    }
    ("not_present".to_string(), None)
}

pub async fn jwt_bearer_handler(req: HttpRequest) -> Result<HttpResponse> {
    // Extract Authorization header
    let auth_header = req.headers().get("Authorization");

    match auth_header {
        Some(auth_header_value) => {
            let auth_str = match auth_header_value.to_str() {
                Ok(s) => s,
                Err(_) => {
                    return Ok(HttpResponse::BadRequest().json(json!({
                        "authenticated": false,
                        "error": "Invalid Authorization header encoding"
                    })));
                }
            };

            // Check if it's a Bearer token
            if !auth_str.starts_with("Bearer ") {
                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", "Bearer"))
                    .json(json!({
                        "authenticated": false,
                        "error": "Authorization header must start with 'Bearer '"
                    })));
            }

            let token = &auth_str[7..]; // Remove "Bearer " prefix

            if token.is_empty() {
                return Ok(HttpResponse::Unauthorized()
                    .append_header(("WWW-Authenticate", "Bearer"))
                    .json(json!({
                        "authenticated": false,
                        "error": "Bearer token is empty"
                    })));
            }

            // Validate JWT structure and decode
            match validate_jwt_structure(token) {
                Ok((header, payload)) => {
                    // Validate expiration
                    let (exp_status, exp_timestamp) = validate_jwt_expiration(&payload);

                    let is_valid = exp_status == "valid" || exp_status == "not_present";

                    // Build payload formatted object with formatted timestamps
                    let mut payload_formatted = HashMap::new();

                    // Add all claims from payload
                    if let Some(payload_obj) = payload.as_object() {
                        for (key, value) in payload_obj {
                            payload_formatted.insert(key.clone(), value.clone());
                        }
                    }

                    // Replace timestamp fields with formatted versions

                    if let Some(iat) = payload.get("iat").and_then(|v| v.as_i64()) {
                        payload_formatted
                            .insert("iat".to_string(), json!(format_unix_timestamp(iat)));
                    }

                    if let Some(exp) = exp_timestamp {
                        payload_formatted
                            .insert("exp".to_string(), json!(format_unix_timestamp(exp)));
                    }

                    if let Some(nbf) = payload.get("nbf").and_then(|v| v.as_i64()) {
                        payload_formatted
                            .insert("nbf".to_string(), json!(format_unix_timestamp(nbf)));
                    }

                    // Build validation status
                    let validation_status = json!({
                        "structure": "valid",
                        "expiration": exp_status
                    });

                    let response_data = json!({
                        "authenticated": is_valid,
                        "token": {
                            "raw": token,
                            "header": header,
                            "payload": payload,
                            "payloadFormatted": payload_formatted,
                            "validationStatus": validation_status
                        }
                    });

                    if is_valid {
                        Ok(HttpResponse::Ok().json(response_data))
                    } else {
                        let mut error_response = response_data;
                        error_response["error"] = json!(match exp_status.as_str() {
                            "expired" => "Token expired",
                            "invalid_format" => "Invalid expiration claim format",
                            _ => "Token validation failed",
                        });
                        Ok(HttpResponse::Unauthorized()
                            .append_header(("WWW-Authenticate", "Bearer"))
                            .json(error_response))
                    }
                }
                Err(validation_error) => {
                    // Try to extract what we can from the malformed token
                    let parts: Vec<&str> = token.split('.').collect();
                    let mut partial_token_info = json!({
                        "raw": token,
                        "parts_count": parts.len()
                    });

                    // Try to decode header if possible
                    if parts.len() >= 2 {
                        if let Ok(header) = decode_header(token) {
                            partial_token_info["header"] = serde_json::to_value(header).unwrap();
                        }
                    }

                    Ok(HttpResponse::Unauthorized()
                        .append_header(("WWW-Authenticate", "Bearer"))
                        .json(json!({
                            "authenticated": false,
                            "error": validation_error,
                            "token": partial_token_info
                        })))
                }
            }
        }
        None => Ok(HttpResponse::Unauthorized()
            .append_header(("WWW-Authenticate", "Bearer"))
            .json(json!({
                "authenticated": false,
                "error": "Missing Authorization header"
            }))),
    }
}
