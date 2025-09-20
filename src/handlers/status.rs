use super::*;

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
    let request_info = extract_request_info(&req, Some(&body));
    
    Ok(HttpResponse::build(status)
        .json(request_info))
}
