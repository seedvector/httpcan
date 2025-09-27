use super::*;
use rand::distributions::WeightedIndex;
use rand::prelude::*;

#[derive(Deserialize)]
pub struct StatusQuery {
    body: Option<String>,
}

#[derive(Debug)]
struct WeightedChoice {
    code: u16,
    weight: f64,
}

fn parse_weighted_codes(codes_str: &str) -> Result<Vec<WeightedChoice>, String> {
    let mut choices = Vec::new();
    
    for choice in codes_str.split(',') {
        let choice = choice.trim();
        
        if choice.is_empty() {
            continue;
        }
        
        let (code_str, weight) = if choice.contains(':') {
            let parts: Vec<&str> = choice.split(':').collect();
            if parts.len() != 2 {
                return Err("Invalid format".to_string());
            }
            (parts[0].trim(), parts[1].trim().parse::<f64>().map_err(|_| "Invalid weight")?)
        } else {
            (choice, 1.0)
        };
        
        let code = code_str.parse::<u16>().map_err(|_| "Invalid status code")?;
        
        if weight < 0.0 {
            return Err("Weight cannot be negative".to_string());
        }
        
        choices.push(WeightedChoice { code, weight });
    }
    
    if choices.is_empty() {
        return Err("No valid status codes found".to_string());
    }
    
    Ok(choices)
}

fn select_weighted_code(choices: &[WeightedChoice]) -> Result<u16, String> {
    if choices.len() == 1 {
        return Ok(choices[0].code);
    }
    
    let weights: Vec<f64> = choices.iter().map(|c| c.weight).collect();
    
    // Check if all weights are zero
    if weights.iter().all(|&w| w == 0.0) {
        return Err("All weights are zero".to_string());
    }
    
    let mut rng = thread_rng();
    
    match WeightedIndex::new(&weights) {
        Ok(dist) => {
            let index = dist.sample(&mut rng);
            Ok(choices[index].code)
        }
        Err(_) => {
            // Fallback to uniform random selection if weights are invalid
            let index = rng.gen_range(0..choices.len());
            Ok(choices[index].code)
        }
    }
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

fn determine_response_content_type(
    req: &HttpRequest,
    has_body: bool,
) -> String {
    // Priority: Accept header > request Content-Type header > default
    
    // 1. Check Accept header first (highest priority)
    if let Some(accept_header) = req.headers().get("accept") {
        let accept_type = parse_accept_header(Some(accept_header));
        if accept_type != "text/plain" {
            return accept_type;
        }
    }
    
    // 2. If request has body and Content-Type header, use that
    if has_body {
        if let Some(content_type_header) = req.headers().get("content-type") {
            if let Ok(content_type_str) = content_type_header.to_str() {
                // Extract just the MIME type part (before any semicolon)
                let mime_type = content_type_str.split(';').next().unwrap_or(content_type_str).trim();
                return mime_type.to_string();
            }
        }
    }
    
    // 3. Default fallback
    "text/plain".to_string()
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
    
    // Parse the status codes (supports both simple and weighted formats)
    let choices = match parse_weighted_codes(&codes_str) {
        Ok(choices) => choices,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid status code"
            })));
        }
    };
    
    // Select a status code (with weights if specified)
    let chosen_code = match select_weighted_code(&choices) {
        Ok(code) => code,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid status code"
            })));
        }
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
            // Determine Content-Type with proper priority: Accept > Content-Type > default
            let content_type = determine_response_content_type(&req, false);
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
    
    // Parse the status codes (supports both simple and weighted formats)
    let choices = match parse_weighted_codes(&codes_str) {
        Ok(choices) => choices,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid status code"
            })));
        }
    };
    
    // Select a status code (with weights if specified)
    let chosen_code = match select_weighted_code(&choices) {
        Ok(code) => code,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid status code"
            })));
        }
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
        let has_request_body = !body.trim().is_empty();
        let custom_body = if has_request_body {
            Some(body)
        } else {
            query.body.clone()
        };
        
        if let Some(custom_body) = custom_body {
            // Determine Content-Type with proper priority: Accept > Content-Type > default
            let content_type = determine_response_content_type(&req, has_request_body);
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

pub async fn status_options_handler(
    _req: HttpRequest,
    _path: web::Path<String>,
) -> Result<HttpResponse> {
    // Return appropriate CORS headers for OPTIONS preflight requests
    Ok(HttpResponse::Ok()
        .append_header(("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, TRACE, OPTIONS"))
        .append_header(("Access-Control-Allow-Headers", "Content-Type, Authorization, Accept, X-Requested-With"))
        .append_header(("Access-Control-Max-Age", "3600"))
        .finish())
}