use super::*;

pub async fn headers_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let headers: HashMap<String, String> = req
        .headers()
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
        .collect();
    
    // Filter out reverse proxy and CDN headers, plus custom exclusions
    let filtered_headers = filter_headers(headers, &config.exclude_headers);
    
    Ok(HttpResponse::Ok().json(json!({
        "headers": sort_hashmap(filtered_headers)
    })))
}

pub async fn ip_handler(req: HttpRequest) -> Result<HttpResponse> {
    let connection_info = req.connection_info();
    let origin = connection_info.realip_remote_addr().unwrap_or("127.0.0.1");
    
    Ok(HttpResponse::Ok().json(json!({
        "origin": origin
    })))
}

pub async fn user_agent_handler(req: HttpRequest) -> Result<HttpResponse> {
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    Ok(HttpResponse::Ok().json(json!({
        "user-agent": user_agent
    })))
}
