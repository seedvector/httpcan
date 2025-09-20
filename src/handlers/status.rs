use super::*;

pub async fn status_handler_get(
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let codes_str = path.into_inner();
    
    // Parse the status codes (can be comma-separated)
    let codes: Vec<u16> = codes_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    
    if codes.is_empty() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid status code"
        })));
    }
    
    // If multiple codes, pick one randomly
    let mut rng = rand::thread_rng();
    let chosen_code = if codes.len() == 1 {
        codes[0]
    } else {
        codes[rng.gen_range(0..codes.len())]
    };
    
    let status = StatusCode::from_u16(chosen_code).unwrap_or(StatusCode::OK);
    let request_info = extract_request_info(&req, None);
    
    Ok(HttpResponse::build(status)
        .json(request_info))
}

pub async fn status_handler(
    req: HttpRequest,
    path: web::Path<String>,
    body: String,
) -> Result<HttpResponse> {
    let codes_str = path.into_inner();
    
    // Parse the status codes (can be comma-separated)
    let codes: Vec<u16> = codes_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    
    if codes.is_empty() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid status code"
        })));
    }
    
    // If multiple codes, pick one randomly
    let mut rng = rand::thread_rng();
    let chosen_code = if codes.len() == 1 {
        codes[0]
    } else {
        codes[rng.gen_range(0..codes.len())]
    };
    
    let status = StatusCode::from_u16(chosen_code).unwrap_or(StatusCode::OK);
    let body_str = if body.is_empty() { None } else { Some(body.as_str()) };
    let request_info = extract_request_info(&req, body_str);
    
    Ok(HttpResponse::build(status)
        .json(request_info))
}
