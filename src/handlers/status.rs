use super::*;

#[derive(Deserialize)]
pub struct StatusQuery {
    body: Option<String>,
}

pub async fn status_handler_get(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<StatusQuery>,
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
        // Check if custom body is provided via query parameter
        if let Some(custom_body) = &query.body {
            // Check Accept header to determine response format
            if let Some(accept_header) = req.headers().get("accept") {
                if let Ok(accept_str) = accept_header.to_str() {
                    if accept_str.contains("application/json") {
                        // Return JSON response with custom body as a field
                        Ok(HttpResponse::build(status).json(json!({
                            "status": chosen_code,
                            "body": custom_body
                        })))
                    } else {
                        // Return plain text response
                        Ok(HttpResponse::build(status)
                            .content_type("text/plain")
                            .body(custom_body.clone()))
                    }
                } else {
                    // Default to plain text if Accept header can't be parsed
                    Ok(HttpResponse::build(status)
                        .content_type("text/plain")
                        .body(custom_body.clone()))
                }
            } else {
                // No Accept header, default to plain text like httpstat.us
                Ok(HttpResponse::build(status)
                    .content_type("text/plain")
                    .body(custom_body.clone()))
            }
        } else {
            // Default behavior - return JSON with status
            Ok(HttpResponse::build(status).json(json!({
                "status": chosen_code
            })))
        }
    }
}

pub async fn status_handler(
    req: HttpRequest,
    path: web::Path<String>,
    body: String,
    query: web::Query<StatusQuery>,
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
        // Priority: 1. Request body, 2. Query parameter, 3. Default
        let custom_body = if !body.trim().is_empty() {
            Some(body)
        } else {
            query.body.clone()
        };
        
        if let Some(custom_body) = custom_body {
            // Check Accept header to determine response format
            if let Some(accept_header) = req.headers().get("accept") {
                if let Ok(accept_str) = accept_header.to_str() {
                    if accept_str.contains("application/json") {
                        // Return JSON response with custom body as a field
                        Ok(HttpResponse::build(status).json(json!({
                            "status": chosen_code,
                            "body": custom_body
                        })))
                    } else {
                        // Return plain text response
                        Ok(HttpResponse::build(status)
                            .content_type("text/plain")
                            .body(custom_body))
                    }
                } else {
                    // Default to plain text if Accept header can't be parsed
                    Ok(HttpResponse::build(status)
                        .content_type("text/plain")
                        .body(custom_body))
                }
            } else {
                // No Accept header, default to plain text like httpstat.us
                Ok(HttpResponse::build(status)
                    .content_type("text/plain")
                    .body(custom_body))
            }
        } else {
            // Default behavior - return JSON with status
            Ok(HttpResponse::build(status).json(json!({
                "status": chosen_code
            })))
        }
    }
}