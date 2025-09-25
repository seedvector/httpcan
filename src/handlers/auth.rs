use super::*;
use std::collections::HashMap;
use md5;
use sha2::{Sha256, Sha512, Digest};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use chrono::DateTime;

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

// Function to calculate digest hash
fn calculate_digest_response(
    username: &str,
    password: &str,
    realm: &str,
    method: &str,
    uri: &str,
    nonce: &str,
    algorithm: &str,
) -> String {
    let ha1 = match algorithm {
        "MD5" => {
            let hash = md5::compute(format!("{}:{}:{}", username, realm, password));
            format!("{:x}", hash)
        }
        "SHA-256" => {
            let mut hasher = Sha256::new();
            hasher.update(format!("{}:{}:{}", username, realm, password));
            format!("{:x}", hasher.finalize())
        }
        "SHA-512" => {
            let mut hasher = Sha512::new();
            hasher.update(format!("{}:{}:{}", username, realm, password));
            format!("{:x}", hasher.finalize())
        }
        _ => return String::new(), // Invalid algorithm
    };

    let ha2 = match algorithm {
        "MD5" => {
            let hash = md5::compute(format!("{}:{}", method, uri));
            format!("{:x}", hash)
        }
        "SHA-256" => {
            let mut hasher = Sha256::new();
            hasher.update(format!("{}:{}", method, uri));
            format!("{:x}", hasher.finalize())
        }
        "SHA-512" => {
            let mut hasher = Sha512::new();
            hasher.update(format!("{}:{}", method, uri));
            format!("{:x}", hasher.finalize())
        }
        _ => return String::new(),
    };

    // For simplicity, we're not implementing qop="auth-int" or other variations
    // Just basic digest: response = MD5(HA1:nonce:HA2)
    match algorithm {
        "MD5" => {
            let hash = md5::compute(format!("{}:{}:{}", ha1, nonce, ha2));
            format!("{:x}", hash)
        }
        "SHA-256" => {
            let mut hasher = Sha256::new();
            hasher.update(format!("{}:{}:{}", ha1, nonce, ha2));
            format!("{:x}", hasher.finalize())
        }
        "SHA-512" => {
            let mut hasher = Sha512::new();
            hasher.update(format!("{}:{}:{}", ha1, nonce, ha2));
            format!("{:x}", hasher.finalize())
        }
        _ => String::new(),
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
        None => {
            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", "Basic realm=\"Fake Realm\""))
                .json(json!({
                    "authenticated": false
                })))
        }
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
        None => {
            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", "Basic realm=\"Fake Realm\""))
                .json(json!({
                    "authenticated": false
                })))
        }
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
        None => {
            Ok(HttpResponse::NotFound().json(json!({
                "authenticated": false
            })))
        }
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
        None => {
            Ok(HttpResponse::NotFound().json(json!({
                "authenticated": false
            })))
        }
    }
}

pub async fn bearer_auth_handler(
    _req: HttpRequest,
    auth: Option<BearerAuth>,
) -> Result<HttpResponse> {
    match auth {
        Some(auth) => {
            Ok(HttpResponse::Ok().json(json!({
                "authenticated": true,
                "token": auth.token()
            })))
        }
        None => {
            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", "Bearer"))
                .json(json!({
                    "authenticated": false
                })))
        }
    }
}

pub async fn digest_auth_handler(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse> {
    let (_qop, expected_user, expected_passwd) = path.into_inner();
    
    let auth_header = req.headers().get("Authorization");
    
    match auth_header {
        Some(auth_header_value) => {
            let auth_str = auth_header_value.to_str().unwrap_or("");
            let digest_params = parse_digest_auth(auth_str);
            
            // Extract required parameters
            let username = digest_params.get("username").cloned().unwrap_or_default();
            let realm = digest_params.get("realm").cloned().unwrap_or_default();
            let nonce = digest_params.get("nonce").cloned().unwrap_or_default();
            let uri = digest_params.get("uri").cloned().unwrap_or_default();
            let response = digest_params.get("response").cloned().unwrap_or_default();
            let algorithm = digest_params.get("algorithm").cloned().unwrap_or("MD5".to_string());
            
            // Verify username matches expected
            if username != expected_user {
                return Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": format!("Username mismatch. Expected '{}' but got '{}'", expected_user, username)
                })));
            }
            
            // Calculate expected response
            let method = req.method().as_str();
            let expected_response = calculate_digest_response(
                &expected_user,
                &expected_passwd,
                &realm,
                method,
                &uri,
                &nonce,
                &algorithm,
            );
            
            // Verify the response hash
            if response == expected_response {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": expected_user
                })))
            } else {
                Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": "Invalid credentials"
                })))
            }
        }
        None => {
            let nonce = format!("{:x}", rand::random::<u64>());
            let auth_header = format!(
                "Digest realm=\"httpbin@{}\", nonce=\"{}\", qop=\"auth\"",
                req.connection_info().host(),
                nonce
            );
            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", auth_header.as_str()))
                .json(json!({
                    "authenticated": false
                })))
        }
    }
}

pub async fn digest_auth_with_algorithm_handler(
    req: HttpRequest,
    path: web::Path<(String, String, String, String)>,
) -> Result<HttpResponse> {
    let (_qop, expected_user, expected_passwd, algorithm) = path.into_inner();
    
    // Validate algorithm parameter - only accept MD5, SHA-256, SHA-512
    if !matches!(algorithm.as_str(), "MD5" | "SHA-256" | "SHA-512") {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid algorithm. Supported algorithms: MD5, SHA-256, SHA-512"
        })));
    }
    
    let auth_header = req.headers().get("Authorization");
    
    match auth_header {
        Some(auth_header_value) => {
            let auth_str = auth_header_value.to_str().unwrap_or("");
            let digest_params = parse_digest_auth(auth_str);
            
            // Extract required parameters
            let username = digest_params.get("username").cloned().unwrap_or_default();
            let realm = digest_params.get("realm").cloned().unwrap_or_default();
            let nonce = digest_params.get("nonce").cloned().unwrap_or_default();
            let uri = digest_params.get("uri").cloned().unwrap_or_default();
            let response = digest_params.get("response").cloned().unwrap_or_default();
            let auth_algorithm = digest_params.get("algorithm").cloned().unwrap_or("MD5".to_string());
            
            // Check if algorithm in Authorization header matches URL parameter
            if auth_algorithm != algorithm {
                return Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": format!("Algorithm mismatch. URL specifies '{}' but Authorization header uses '{}'", algorithm, auth_algorithm)
                })));
            }
            
            // Verify username matches expected
            if username != expected_user {
                return Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": format!("Username mismatch. Expected '{}' but got '{}'", expected_user, username)
                })));
            }
            
            // Calculate expected response
            let method = req.method().as_str();
            let expected_response = calculate_digest_response(
                &expected_user,
                &expected_passwd,
                &realm,
                method,
                &uri,
                &nonce,
                &algorithm,
            );
            
            // Verify the response hash
            if response == expected_response {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": expected_user,
                    "algorithm": algorithm
                })))
            } else {
                Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": "Invalid credentials"
                })))
            }
        }
        None => {
            let nonce = format!("{:x}", rand::random::<u64>());
            let auth_header = format!(
                "Digest realm=\"httpbin@{}\", nonce=\"{}\", qop=\"auth\", algorithm=\"{}\"",
                req.connection_info().host(),
                nonce,
                algorithm
            );
            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", auth_header.as_str()))
                .json(json!({
                    "authenticated": false
                })))
        }
    }
}

pub async fn digest_auth_full_handler(
    req: HttpRequest,
    path: web::Path<(String, String, String, String, String)>,
) -> Result<HttpResponse> {
    let (_qop, _user, _passwd, algorithm, _stale_after) = path.into_inner();
    
    // Validate algorithm parameter - only accept MD5, SHA-256, SHA-512
    if !matches!(algorithm.as_str(), "MD5" | "SHA-256" | "SHA-512") {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid algorithm. Supported algorithms: MD5, SHA-256, SHA-512"
        })));
    }
    
    let auth_header = req.headers().get("Authorization");
    
    match auth_header {
        Some(auth_header_value) => {
            let auth_str = auth_header_value.to_str().unwrap_or("");
            let digest_params = parse_digest_auth(auth_str);
            
            // Check if algorithm in Authorization header matches URL parameter
            if let Some(auth_algorithm) = digest_params.get("algorithm") {
                if auth_algorithm != &algorithm {
                    return Ok(HttpResponse::Unauthorized().json(json!({
                        "authenticated": false,
                        "error": format!("Algorithm mismatch. URL specifies '{}' but Authorization header uses '{}'", algorithm, auth_algorithm)
                    })));
                }
            } else if algorithm != "MD5" {
                // If no algorithm specified in auth header, it defaults to MD5
                return Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": format!("Algorithm mismatch. URL specifies '{}' but Authorization header defaults to 'MD5'", algorithm)
                })));
            }
            
            // Extract additional required parameters
            let username = digest_params.get("username").cloned().unwrap_or_default();
            let realm = digest_params.get("realm").cloned().unwrap_or_default();
            let nonce = digest_params.get("nonce").cloned().unwrap_or_default();
            let uri = digest_params.get("uri").cloned().unwrap_or_default();
            let response = digest_params.get("response").cloned().unwrap_or_default();
            
            // Verify username matches expected
            if username != _user {
                return Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": format!("Username mismatch. Expected '{}' but got '{}'", _user, username)
                })));
            }
            
            // Calculate expected response
            let method = req.method().as_str();
            let expected_response = calculate_digest_response(
                &_user,
                &_passwd,
                &realm,
                method,
                &uri,
                &nonce,
                &algorithm,
            );
            
            // Verify the response hash
            if response == expected_response {
                Ok(HttpResponse::Ok().json(json!({
                    "authenticated": true,
                    "user": _user,
                    "algorithm": algorithm
                })))
            } else {
                Ok(HttpResponse::Unauthorized().json(json!({
                    "authenticated": false,
                    "error": "Invalid credentials"
                })))
            }
        }
        None => {
            let nonce = format!("{:x}", rand::random::<u64>());
            let auth_header = format!(
                "Digest realm=\"httpbin@{}\", nonce=\"{}\", qop=\"auth\", algorithm=\"{}\"",
                req.connection_info().host(),
                nonce,
                algorithm
            );
            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", auth_header.as_str()))
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

    // Try to decode payload (without verification)
    let mut validation = Validation::default();
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    validation.required_spec_claims = std::collections::HashSet::new();

    let payload = match decode::<Claims>(token, &DecodingKey::from_secret(&[]), &validation) {
        Ok(token_data) => token_data.claims.standard_claims,
        Err(e) => return Err(format!("Invalid JWT payload: {}", e)),
    };

    Ok((serde_json::to_value(header).unwrap(), serde_json::to_value(payload).unwrap()))
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
                        payload_formatted.insert("iat".to_string(), 
                            json!(format_unix_timestamp(iat)));
                    }
                    
                    if let Some(exp) = exp_timestamp {
                        payload_formatted.insert("exp".to_string(), 
                            json!(format_unix_timestamp(exp)));
                    }
                    
                    if let Some(nbf) = payload.get("nbf").and_then(|v| v.as_i64()) {
                        payload_formatted.insert("nbf".to_string(), 
                            json!(format_unix_timestamp(nbf)));
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
                            _ => "Token validation failed"
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
        None => {
            Ok(HttpResponse::Unauthorized()
                .append_header(("WWW-Authenticate", "Bearer"))
                .json(json!({
                    "authenticated": false,
                    "error": "Missing Authorization header"
                })))
        }
    }
}
