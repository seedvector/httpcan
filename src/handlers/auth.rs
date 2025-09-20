use super::*;

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
    let (_qop, _user, _passwd) = path.into_inner();
    
    // Simplified digest auth implementation
    let auth_header = req.headers().get("Authorization");
    
    match auth_header {
        Some(_) => {
            Ok(HttpResponse::Ok().json(json!({
                "authenticated": true,
                "user": _user
            })))
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
    let (_qop, _user, _passwd, algorithm) = path.into_inner();
    
    let auth_header = req.headers().get("Authorization");
    
    match auth_header {
        Some(_) => {
            Ok(HttpResponse::Ok().json(json!({
                "authenticated": true,
                "user": _user
            })))
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
    
    let auth_header = req.headers().get("Authorization");
    
    match auth_header {
        Some(_) => {
            Ok(HttpResponse::Ok().json(json!({
                "authenticated": true,
                "user": _user
            })))
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
