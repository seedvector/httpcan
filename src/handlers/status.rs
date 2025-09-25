use super::*;

#[derive(Deserialize)]
pub struct StatusQuery {
    body: Option<String>,
}

fn parse_accept_header(accept_header: Option<&actix_web::http::header::HeaderValue>) -> String {
    if let Some(accept) = accept_header {
        if let Ok(accept_str) = accept.to_str() {
            // Parse Accept header for the first acceptable MIME type
            // Handle weighted preferences (q=0.9) and multiple types
            let mut best_type = "text/plain";
            let mut best_weight = 0.0;
            
            for media_range in accept_str.split(',') {
                let media_range = media_range.trim();
                let parts: Vec<&str> = media_range.split(';').collect();
                let mime_type = parts[0].trim();
                
                // Extract quality value (default is 1.0)
                let mut quality = 1.0;
                for part in parts.iter().skip(1) {
                    if let Some(q_value) = part.trim().strip_prefix("q=") {
                        if let Ok(q) = q_value.parse::<f32>() {
                            quality = q;
                        }
                    }
                }
                
                // Skip if quality is 0
                if quality == 0.0 {
                    continue;
                }
                
                // Accept wildcard types
                if mime_type == "*/*" || mime_type == "text/*" {
                    if quality > best_weight {
                        best_type = "text/plain";
                        best_weight = quality;
                    }
                } else if quality > best_weight {
                    best_type = mime_type;
                    best_weight = quality;
                }
            }
            
            best_type.to_string()
        } else {
            "text/plain".to_string()
        }
    } else {
        "text/plain".to_string()
    }
}

fn format_response_body(content: &str, content_type: &str) -> (String, String) {
    match content_type {
        "application/json" => {
            // Try to parse as JSON first
            match serde_json::from_str::<serde_json::Value>(content) {
                Ok(_) => {
                    // Valid JSON, return as is
                    (content.to_string(), content_type.to_string())
                }
                Err(_) => {
                    // Invalid JSON, wrap in a JSON object
                    let wrapped = json!({ "body": content });
                    (wrapped.to_string(), content_type.to_string())
                }
            }
        }
        _ => {
            // For all other content types, return as plain text
            (content.to_string(), content_type.to_string())
        }
    }
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
            // Parse Accept header to determine Content-Type
            let content_type = parse_accept_header(req.headers().get("accept"));
            let (formatted_body, final_content_type) = format_response_body(custom_body, &content_type);
            
            Ok(HttpResponse::build(status)
                .content_type(final_content_type)
                .body(formatted_body))
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
            // Parse Accept header to determine Content-Type
            let content_type = parse_accept_header(req.headers().get("accept"));
            let (formatted_body, final_content_type) = format_response_body(&custom_body, &content_type);
            
            Ok(HttpResponse::build(status)
                .content_type(final_content_type)
                .body(formatted_body))
        } else {
            // Default behavior - return JSON with status
            Ok(HttpResponse::build(status).json(json!({
                "status": chosen_code
            })))
        }
    }
}