use super::*;

#[derive(Deserialize)]
pub struct RedirectToQuery {
    url: String,
    status_code: Option<u16>,
}

#[derive(Deserialize)]
pub struct RedirectQuery {
    absolute: Option<String>,
}

#[derive(Deserialize)]
pub struct RedirectToForm {
    url: String,
    status_code: Option<u16>,
}

pub async fn redirect_handler(
    req: HttpRequest,
    path: web::Path<usize>,
    query: web::Query<RedirectQuery>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    
    if n == 0 {
        return Err(actix_web::error::ErrorBadRequest("n must be greater than 0"));
    }
    
    let absolute = query.absolute.as_ref()
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false);
    
    if n == 1 {
        // Final redirect to /get
        if absolute {
            let host = req.connection_info().host().to_string();
            let scheme = if req.connection_info().scheme() == "https" { "https" } else { "http" };
            Ok(HttpResponse::Found()
                .append_header(("Location", format!("{}://{}/get", scheme, host)))
                .body(""))
        } else {
            Ok(HttpResponse::Found()
                .append_header(("Location", "/get"))
                .body(""))
        }
    } else {
        // Redirect to the next step
        if absolute {
            let host = req.connection_info().host().to_string();
            let scheme = if req.connection_info().scheme() == "https" { "https" } else { "http" };
            Ok(HttpResponse::Found()
                .append_header(("Location", format!("{}://{}/absolute-redirect/{}", scheme, host, n - 1)))
                .body(""))
        } else {
            Ok(HttpResponse::Found()
                .append_header(("Location", format!("/relative-redirect/{}", n - 1)))
                .body(""))
        }
    }
}

pub async fn relative_redirect_handler(
    _req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    
    if n == 0 {
        return Err(actix_web::error::ErrorBadRequest("n must be greater than 0"));
    }
    
    if n == 1 {
        // Final redirect to /get
        Ok(HttpResponse::Found()
            .append_header(("Location", "/get"))
            .body(""))
    } else {
        // Relative redirect to the next step
        Ok(HttpResponse::Found()
            .append_header(("Location", format!("/relative-redirect/{}", n - 1)))
            .body(""))
    }
}

pub async fn absolute_redirect_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    
    if n == 0 {
        return Err(actix_web::error::ErrorBadRequest("n must be greater than 0"));
    }
    
    let host = req.connection_info().host().to_string();
    let scheme = if req.connection_info().scheme() == "https" { "https" } else { "http" };
    
    if n == 1 {
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
    _req: HttpRequest,
    query: web::Query<RedirectToQuery>,
) -> Result<HttpResponse> {
    let mut status_code = 302;
    if let Some(code) = query.status_code {
        if code >= 300 && code < 400 {
            status_code = code;
        }
    }
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);
    
    // Ensure UTF-8 encoding of the URL like httpbin does
    let location = query.url.as_bytes();
    
    Ok(HttpResponse::build(status)
        .append_header(("Location", String::from_utf8_lossy(location).to_string()))
        .body(""))
}

pub async fn redirect_to_handler(
    _req: HttpRequest,
    form: web::Form<RedirectToForm>,
) -> Result<HttpResponse> {
    let mut status_code = 302;
    if let Some(code) = form.status_code {
        if code >= 300 && code < 400 {
            status_code = code;
        }
    }
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);
    
    // Ensure UTF-8 encoding of the URL like httpbin does
    let location = form.url.as_bytes();
    
    Ok(HttpResponse::build(status)
        .append_header(("Location", String::from_utf8_lossy(location).to_string()))
        .body(""))
}
