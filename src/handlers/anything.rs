use super::*;

pub async fn anything_handler_get(req: HttpRequest) -> Result<HttpResponse> {
    let request_info = extract_request_info(&req, None);
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_handler(req: HttpRequest, body: String) -> Result<HttpResponse> {
    let request_info = extract_request_info(&req, Some(&body));
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_with_param_handler_get(
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None);
    // Add the path parameter to the response
    request_info.args.insert("anything".to_string(), path.into_inner());
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_with_param_handler(
    req: HttpRequest,
    path: web::Path<String>,
    body: String,
) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, Some(&body));
    // Add the path parameter to the response
    request_info.args.insert("anything".to_string(), path.into_inner());
    Ok(HttpResponse::Ok().json(request_info))
}
