use super::*;
use serde_json::Value;
use crate::handlers::utils::{parse_multi_value_header, http_date, generate_etag};

pub async fn cache_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let if_modified_since = req.headers().get("If-Modified-Since");
    let if_none_match = req.headers().get("If-None-Match");
    
    // Generate dynamic values like httpbin
    let last_modified = http_date();
    let etag = generate_etag();
    
    if if_modified_since.is_some() || if_none_match.is_some() {
        // 304 response should include cache-related headers
        Ok(HttpResponse::NotModified()
            .append_header(("Last-Modified", last_modified))
            .append_header(("ETag", etag))
            .finish())
    } else {
        let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok()
            .append_header(("Last-Modified", last_modified))
            .append_header(("ETag", etag))
            .json(request_info))
    }
}

pub async fn cache_control_handler(
    req: HttpRequest,
    path: web::Path<u32>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let seconds = path.into_inner();
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    
    Ok(HttpResponse::Ok()
        .append_header(("Cache-Control", format!("public, max-age={}", seconds)))
        .json(request_info))
}

pub async fn etag_handler(
    req: HttpRequest,
    path: web::Path<String>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let etag = path.into_inner();
    
    let if_none_match_values = parse_multi_value_header(req.headers().get("If-None-Match"));
    let if_match_values = parse_multi_value_header(req.headers().get("If-Match"));
    
    // Check If-None-Match first (httpbin logic)
    if !if_none_match_values.is_empty() {
        // Check for exact match or wildcard
        let etag_quoted = format!("\"{}\"", etag);
        if if_none_match_values.contains(&etag) || 
           if_none_match_values.contains(&etag_quoted) ||
           if_none_match_values.contains(&"*".to_string()) {
            return Ok(HttpResponse::NotModified()
                .append_header(("ETag", format!("\"{}\"", etag)))
                .finish());
        }
    }
    // Only check If-Match if If-None-Match was not present (httpbin uses elif)
    else if !if_match_values.is_empty() {
        let etag_quoted = format!("\"{}\"", etag);
        if !if_match_values.contains(&etag) && 
           !if_match_values.contains(&etag_quoted) &&
           !if_match_values.contains(&"*".to_string()) {
            return Ok(HttpResponse::PreconditionFailed().finish());
        }
    }
    
    // Normal response with ETag
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    Ok(HttpResponse::Ok()
        .append_header(("ETag", format!("\"{}\"", etag)))
        .json(request_info))
}

pub async fn response_headers_get_handler(
    _req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let mut response = HttpResponse::Ok();
    
    // First, add all query parameters as actual response headers
    for (key, value) in query.iter() {
        response.append_header((key.as_str(), value.as_str()));
    }
    
    // Prepare the headers map that will be in the JSON response
    let mut headers_map: HashMap<String, Value> = HashMap::new();
    
    // Add query parameters to the JSON response
    for (key, value) in query.iter() {
        headers_map.insert(key.clone(), Value::String(value.clone()));
    }
    
    // Add the standard headers that httpbin includes in the JSON response
    headers_map.insert("content-type".to_string(), Value::String("application/json".to_string()));
    
    // Calculate content length for the final JSON
    let temp_json = serde_json::to_string(&headers_map)?;
    let final_content_length = temp_json.len().to_string();
    headers_map.insert("content-length".to_string(), Value::String(final_content_length));
    
    Ok(response.json(headers_map))
}

pub async fn response_headers_post_handler(
    _req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    _body: String,
) -> Result<HttpResponse> {
    let mut response = HttpResponse::Ok();
    
    // First, add all query parameters as actual response headers
    for (key, value) in query.iter() {
        response.append_header((key.as_str(), value.as_str()));
    }
    
    // Prepare the headers map that will be in the JSON response
    let mut headers_map: HashMap<String, Value> = HashMap::new();
    
    // Add query parameters to the JSON response
    for (key, value) in query.iter() {
        headers_map.insert(key.clone(), Value::String(value.clone()));
    }
    
    // Add the standard headers that httpbin includes in the JSON response
    headers_map.insert("content-type".to_string(), Value::String("application/json".to_string()));
    
    // Calculate content length for the final JSON
    let temp_json = serde_json::to_string(&headers_map)?;
    let final_content_length = temp_json.len().to_string();
    headers_map.insert("content-length".to_string(), Value::String(final_content_length));
    
    Ok(response.json(headers_map))
}
