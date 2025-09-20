use super::*;

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
    
    for (name, value) in query.iter() {
        let cookie = Cookie::build(name, value)
            .path("/")
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
    
    let cookie = Cookie::build(&name, &value)
        .path("/")
        .finish();
    
    Ok(HttpResponse::Found()
        .cookie(cookie)
        .append_header(("Location", "/cookies"))
        .body("Redirecting to /cookies"))
}

pub async fn cookies_delete_handler(
    req: HttpRequest,
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
