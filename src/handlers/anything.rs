use super::*;


pub async fn anything_handler_get(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_handler(req: HttpRequest, payload: web::Payload, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let request_info = process_request_payload(&req, payload, &config.exclude_headers, None).await?;
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_with_param_handler_get(
    req: HttpRequest,
    path: web::Path<String>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    // Add the path parameter to the response (use "anything" as key for consistency)
    request_info.args.insert("anything".to_string(), serde_json::Value::String(path.into_inner()));
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_with_param_handler(
    req: HttpRequest,
    path: web::Path<String>,
    payload: web::Payload,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let request_info = process_request_payload(&req, payload, &config.exclude_headers, Some(path.into_inner())).await?;
    Ok(HttpResponse::Ok().json(request_info))
}

