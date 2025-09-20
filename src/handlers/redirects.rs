use super::*;

#[derive(Deserialize)]
pub struct RedirectToQuery {
    url: String,
    status_code: Option<u16>,
}

#[derive(Deserialize)]
pub struct RedirectToForm {
    url: String,
    status_code: Option<u16>,
}

pub async fn redirect_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    
    if n <= 1 {
        // Final redirect to /get
        Ok(HttpResponse::Found()
            .append_header(("Location", "/get"))
            .body(""))
    } else {
        // Redirect to the next step
        Ok(HttpResponse::Found()
            .append_header(("Location", format!("/redirect/{}", n - 1)))
            .body(""))
    }
}

pub async fn relative_redirect_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    
    if n <= 1 {
        // Final redirect to /get
        Ok(HttpResponse::Found()
            .append_header(("Location", "/get"))
            .body(""))
    } else {
        // Relative redirect to the next step
        Ok(HttpResponse::Found()
            .append_header(("Location", format!("relative-redirect/{}", n - 1)))
            .body(""))
    }
}

pub async fn absolute_redirect_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    let host = req.connection_info().host().to_string();
    let scheme = if req.connection_info().scheme() == "https" { "https" } else { "http" };
    
    if n <= 1 {
        // Final redirect to /get
        Ok(HttpResponse::Found()
            .append_header(("Location", format!("{}://{}/get", scheme, host)))
            .body(""))
    } else {
        // Absolute redirect to the next step
        Ok(HttpResponse::Found()
            .append_header(("Location", format!("{}://{}/absolute-redirect/{}", scheme, host, n - 1)))
            .body(""))
    }
}

pub async fn redirect_to_handler_get(
    req: HttpRequest,
    query: web::Query<RedirectToQuery>,
) -> Result<HttpResponse> {
    let status_code = query.status_code.unwrap_or(302);
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);
    
    Ok(HttpResponse::build(status)
        .append_header(("Location", query.url.clone()))
        .body(""))
}

pub async fn redirect_to_handler(
    req: HttpRequest,
    form: web::Form<RedirectToForm>,
) -> Result<HttpResponse> {
    let status_code = form.status_code.unwrap_or(302);
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);
    
    Ok(HttpResponse::build(status)
        .append_header(("Location", form.url.clone()))
        .body(""))
}
