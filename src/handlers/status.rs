use super::*;

pub async fn status_handler_get(
    _req: HttpRequest,
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
    
    // HTTP status codes that should not have a response body
    let should_be_empty = match chosen_code {
        // 1xx Informational responses
        100..=199 => true,
        // 204 No Content
        204 => true,
        // 304 Not Modified  
        304 => true,
        _ => false,
    };
    
    if should_be_empty {
        Ok(HttpResponse::build(status).finish())
    } else {
        Ok(HttpResponse::build(status).json(json!({
            "status": chosen_code
        })))
    }
}

pub async fn status_handler(
    _req: HttpRequest,
    path: web::Path<String>,
    _body: String,
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
    
    // HTTP status codes that should not have a response body
    let should_be_empty = match chosen_code {
        // 1xx Informational responses
        100..=199 => true,
        // 204 No Content
        204 => true,
        // 304 Not Modified  
        304 => true,
        _ => false,
    };
    
    if should_be_empty {
        Ok(HttpResponse::build(status).finish())
    } else {
        Ok(HttpResponse::build(status).json(json!({
            "status": chosen_code
        })))
    }
}
