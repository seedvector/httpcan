use super::*;
use serde_json::Value;

pub async fn cache_handler(req: HttpRequest) -> Result<HttpResponse> {
    let if_modified_since = req.headers().get("If-Modified-Since");
    let if_none_match = req.headers().get("If-None-Match");
    
    if if_modified_since.is_some() || if_none_match.is_some() {
        Ok(HttpResponse::NotModified().finish())
    } else {
        let mut request_info = extract_request_info(&req, None);
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok()
            .append_header(("Cache-Control", "public, max-age=60"))
            .append_header(("ETag", "\"etag\""))
            .append_header(("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT"))
            .json(request_info))
    }
}

pub async fn cache_control_handler(
    req: HttpRequest,
    path: web::Path<u32>,
) -> Result<HttpResponse> {
    let seconds = path.into_inner();
    let mut request_info = extract_request_info(&req, None);
    fix_request_info_url(&req, &mut request_info);
    
    Ok(HttpResponse::Ok()
        .append_header(("Cache-Control", format!("public, max-age={}", seconds)))
        .json(request_info))
}

pub async fn etag_handler(
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let etag = path.into_inner();
    
    let if_none_match = req.headers().get("If-None-Match");
    let if_match = req.headers().get("If-Match");
    
    if let Some(if_none_match_value) = if_none_match {
        if if_none_match_value.to_str().unwrap_or("").contains(&etag) {
            return Ok(HttpResponse::NotModified().finish());
        }
    }
    
    if let Some(if_match_value) = if_match {
        if !if_match_value.to_str().unwrap_or("").contains(&etag) {
            return Ok(HttpResponse::PreconditionFailed().finish());
        }
    }
    
    let mut request_info = extract_request_info(&req, None);
    fix_request_info_url(&req, &mut request_info);
    Ok(HttpResponse::Ok()
        .append_header(("ETag", format!("\"{}\"", etag)))
        .json(request_info))
}

pub async fn response_headers_get_handler(
    req: HttpRequest,
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
    headers_map.insert("Content-type".to_string(), Value::String("application/json".to_string()));
    
    // Calculate content length for the final JSON
    let temp_json = serde_json::to_string(&headers_map)?;
    let final_content_length = temp_json.len().to_string();
    headers_map.insert("Content-length".to_string(), Value::String(final_content_length));
    
    Ok(response.json(headers_map))
}

pub async fn response_headers_post_handler(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    body: String,
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
    headers_map.insert("Content-Type".to_string(), Value::String("application/json".to_string()));
    
    // Calculate content length for the final JSON
    let temp_json = serde_json::to_string(&headers_map)?;
    let final_content_length = temp_json.len().to_string();
    headers_map.insert("Content-Length".to_string(), Value::String(final_content_length));
    
    Ok(response.json(headers_map))
}
