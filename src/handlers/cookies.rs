use super::*;

/// Determines if cookies should be secure based on the request
/// Returns true if the request is over HTTPS or if X-Forwarded-Proto is https
fn secure_cookie(req: &HttpRequest) -> bool {
    // Check if the connection is HTTPS
    if req.connection_info().scheme() == "https" {
        return true;
    }

    // Check X-Forwarded-Proto header for proxy scenarios
    if let Some(proto_header) = req.headers().get("X-Forwarded-Proto") {
        if let Ok(proto_str) = proto_header.to_str() {
            if proto_str.to_lowercase() == "https" {
                return true;
            }
        }
    }

    // Check X-Forwarded-Ssl header
    if let Some(ssl_header) = req.headers().get("X-Forwarded-Ssl") {
        if let Ok(ssl_str) = ssl_header.to_str() {
            if ssl_str.to_lowercase() == "on" {
                return true;
            }
        }
    }

    false
}

pub async fn cookies_handler(req: HttpRequest) -> Result<HttpResponse> {
    let mut cookies = HashMap::new();

    if let Some(cookie_header) = req.headers().get("Cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie_pair in cookie_str.split(';') {
                let cookie_pair = cookie_pair.trim();
                if let Some((name, value)) = cookie_pair.split_once('=') {
                    cookies.insert(name.trim().to_string(), value.trim().to_string());
                }
            }
        }
    }

    Ok(HttpResponse::Ok().json(json!({
        "cookies": cookies
    })))
}

/// Reserved query keys that configure cookie attributes instead of naming a cookie.
const COOKIE_ATTR_KEYS: &[&str] = &[
    "httponly", "secure", "samesite", "domain", "max_age", "path",
];

pub async fn cookies_set_handler(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let mut response = HttpResponse::Found();
    let default_secure = secure_cookie(&req);

    let httponly = matches!(
        query.get("httponly").map(|s| s.to_lowercase()).as_deref(),
        Some("true") | Some("1")
    );
    let secure = query
        .get("secure")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(default_secure);
    let samesite = query.get("samesite").map(|s| s.to_lowercase());
    let domain = query.get("domain").cloned();
    let path = query
        .get("path")
        .cloned()
        .unwrap_or_else(|| "/".to_string());
    let max_age = query.get("max_age").and_then(|s| s.parse::<i64>().ok());

    for (name, value) in query.iter() {
        if COOKIE_ATTR_KEYS.contains(&name.as_str()) {
            continue;
        }
        let mut cookie = Cookie::build(name, value).path(path.clone()).secure(secure);
        if httponly {
            cookie = cookie.http_only(true);
        }
        if let Some(ss) = &samesite {
            if let Some(parsed) = match ss.as_str() {
                "strict" => Some(actix_web::cookie::SameSite::Strict),
                "lax" => Some(actix_web::cookie::SameSite::Lax),
                "none" => Some(actix_web::cookie::SameSite::None),
                _ => None,
            } {
                cookie = cookie.same_site(parsed);
            }
        }
        if let Some(d) = &domain {
            cookie = cookie.domain(d.clone());
        }
        if let Some(ma) = max_age {
            cookie = cookie.max_age(actix_web::cookie::time::Duration::seconds(ma));
        }
        response.cookie(cookie.finish());
    }

    Ok(response
        .append_header(("Location", "/cookies"))
        .body("Redirecting to /cookies"))
}

pub async fn cookies_set_named_handler(
    req: HttpRequest,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse> {
    let (name, value) = path.into_inner();
    let is_secure = secure_cookie(&req);

    let cookie = Cookie::build(&name, &value)
        .path("/")
        .secure(is_secure)
        .finish();

    Ok(HttpResponse::Found()
        .cookie(cookie)
        .append_header(("Location", "/cookies"))
        .body("Redirecting to /cookies"))
}

pub async fn cookies_delete_handler(
    _req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let mut response = HttpResponse::Found();

    for (name, _value) in query.iter() {
        let cookie = Cookie::build(name, "")
            .path("/")
            .max_age(actix_web::cookie::time::Duration::seconds(0))
            .finish();
        response.cookie(cookie);
    }

    Ok(response
        .append_header(("Location", "/cookies"))
        .body("Redirecting to /cookies"))
}
