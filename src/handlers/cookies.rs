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

pub async fn cookies_set_handler(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let mut response = HttpResponse::Found();
    let is_secure = secure_cookie(&req);
    
    for (name, value) in query.iter() {
        let cookie = Cookie::build(name, value)
            .path("/")
            .secure(is_secure)
            .finish();
        response.cookie(cookie);
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
