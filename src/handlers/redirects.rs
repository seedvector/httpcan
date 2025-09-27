use super::*;
use std::collections::HashMap;

// Helper function to create case-insensitive parameter map
fn to_case_insensitive_map(params: &web::Query<HashMap<String, String>>) -> HashMap<String, String> {
    let mut case_insensitive = HashMap::new();
    for (key, value) in params.iter() {
        case_insensitive.insert(key.to_lowercase(), value.clone());
    }
    case_insensitive
}

fn to_case_insensitive_form_map(params: &web::Form<HashMap<String, String>>) -> HashMap<String, String> {
    let mut case_insensitive = HashMap::new();
    for (key, value) in params.iter() {
        case_insensitive.insert(key.to_lowercase(), value.clone());
    }
    case_insensitive
}

#[derive(Deserialize)]
pub struct RedirectQuery {
    absolute: Option<String>,
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
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let params = to_case_insensitive_map(&query);
    
    let url = params.get("url")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing required parameter: url"))?;
    
    let mut status_code = 302u16;
    if let Some(status_str) = params.get("status_code") {
        if let Ok(code) = status_str.parse::<u16>() {
            if code >= 300 && code < 400 {
                status_code = code;
            }
        }
    }
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);
    
    // Match httpbin's exact UTF-8 encoding behavior
    let location_bytes = url.as_bytes();
    
    Ok(HttpResponse::build(status)
        .append_header(("Location", location_bytes))
        .body(""))
}

pub async fn redirect_to_handler(
    _req: HttpRequest,
    form: web::Form<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let params = to_case_insensitive_form_map(&form);
    
    let url = params.get("url")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing required parameter: url"))?;
    
    let mut status_code = 302u16;
    if let Some(status_str) = params.get("status_code") {
        if let Ok(code) = status_str.parse::<u16>() {
            if code >= 300 && code < 400 {
                status_code = code;
            }
        }
    }
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);
    
    // Match httpbin's exact UTF-8 encoding behavior
    let location_bytes = url.as_bytes();
    
    Ok(HttpResponse::build(status)
        .append_header(("Location", location_bytes))
        .body(""))
}
