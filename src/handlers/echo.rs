use actix_web::{web, HttpRequest, HttpResponse, Result};
use futures_util::StreamExt;
use actix_web::web::BytesMut;

/// Echo handler for GET requests - returns empty body or helpful message
pub async fn echo_handler_get(req: HttpRequest) -> Result<HttpResponse> {
    let mut response = HttpResponse::Ok();
    
    // Mirror all headers from request to response, except those that should be set by server
    for (name, value) in req.headers() {
        let header_name = name.as_str().to_lowercase();
        
        // Skip headers that must be controlled by the server
        if !should_skip_header(&header_name) {
            response.append_header((name.clone(), value.clone()));
        }
    }
    
    // For GET requests, return empty body since there's no request body to echo
    Ok(response.body(""))
}

/// Universal echo handler for methods with request body (POST, PUT, PATCH, DELETE)
pub async fn echo_handler(req: HttpRequest, mut payload: web::Payload) -> Result<HttpResponse> {
    // Read the entire request body
    let mut body = BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        body.extend_from_slice(&chunk);
    }
    
    let mut response = HttpResponse::Ok();
    
    // Mirror all headers from request to response, except those that should be set by server
    for (name, value) in req.headers() {
        let header_name = name.as_str().to_lowercase();
        
        // Skip headers that must be controlled by the server
        if !should_skip_header(&header_name) {
            response.append_header((name.clone(), value.clone()));
        }
    }
    
    // Return the exact request body as response body
    Ok(response.body(body.freeze()))
}

/// Determines if a header should be skipped when mirroring from request to response
fn should_skip_header(header_name: &str) -> bool {
    match header_name {
        // Headers that must be controlled by the server
        "content-length" |
        "transfer-encoding" |
        "connection" |
        "upgrade" |
        "server" |
        "date" |
        
        // Headers that don't make sense to mirror in a response
        "host" |
        "user-agent" |
        "accept" |
        "accept-encoding" |
        "accept-language" |
        "accept-charset" |
        "authorization" |
        "cookie" |
        "if-match" |
        "if-none-match" |
        "if-modified-since" |
        "if-unmodified-since" |
        "if-range" |
        "range" |
        "referer" |
        "origin" |
        "dnt" |
        "upgrade-insecure-requests" => true,
        
        _ => false,
    }
}
