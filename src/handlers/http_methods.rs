use super::*;

pub async fn get_handler(req: HttpRequest) -> Result<HttpResponse> {
    let request_info = extract_request_info(&req, None);
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn post_handler(req: HttpRequest, body: String) -> Result<HttpResponse> {
    let request_info = extract_request_info(&req, Some(&body));
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn put_handler(req: HttpRequest, body: String) -> Result<HttpResponse> {
    let request_info = extract_request_info(&req, Some(&body));
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn patch_handler(req: HttpRequest, body: String) -> Result<HttpResponse> {
    let request_info = extract_request_info(&req, Some(&body));
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn delete_handler(req: HttpRequest, body: String) -> Result<HttpResponse> {
    let request_info = extract_request_info(&req, Some(&body));
    Ok(HttpResponse::Ok().json(request_info))
}
