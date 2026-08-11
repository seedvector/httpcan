use super::*;
use actix_web::http::header::HeaderName;

pub async fn get_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let request_info = extract_get_request_info(&req, &config.exclude_headers);
    Ok(HttpResponse::Ok().json(request_info))
}

// Universal handler for body-containing requests (POST, PUT, PATCH, DELETE)
// Returns HTTPBin compatible format (without method and user_agent fields)
async fn universal_body_handler_httpbin(
    req: HttpRequest,
    payload: web::Payload,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type
        .to_lowercase()
        .starts_with("multipart/form-data")
    {
        // Handle multipart form data
        let multipart = Multipart::new(req.headers(), payload);
        match extract_request_info_multipart(&req, multipart, &config.exclude_headers).await {
            Ok(mut request_info) => {
                fix_request_info_url(&req, &mut request_info);
                let http_methods_info = to_http_methods_format(request_info);
                Ok(HttpResponse::Ok().json(http_methods_info))
            }
            Err(_) => {
                let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
                fix_request_info_url(&req, &mut request_info);
                let http_methods_info = to_http_methods_format(request_info);
                Ok(HttpResponse::Ok().json(http_methods_info))
            }
        }
    } else {
        // Handle other content types (JSON, form-urlencoded, etc.)
        use actix_web::web::BytesMut;
        use futures_util::StreamExt;

        let mut body = BytesMut::new();
        let mut payload = payload;
        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            body.extend_from_slice(&chunk);
        }

        let body_string = String::from_utf8_lossy(&body);
        let mut request_info =
            extract_request_info(&req, Some(&body_string), &config.exclude_headers);
        fix_request_info_url(&req, &mut request_info);
        let http_methods_info = to_http_methods_format(request_info);
        Ok(HttpResponse::Ok().json(http_methods_info))
    }
}

pub async fn post_handler(
    req: HttpRequest,
    payload: web::Payload,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    universal_body_handler_httpbin(req, payload, config).await
}

pub async fn put_handler(
    req: HttpRequest,
    payload: web::Payload,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    universal_body_handler_httpbin(req, payload, config).await
}

pub async fn patch_handler(
    req: HttpRequest,
    payload: web::Payload,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    universal_body_handler_httpbin(req, payload, config).await
}

pub async fn delete_handler(
    req: HttpRequest,
    payload: web::Payload,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    universal_body_handler_httpbin(req, payload, config).await
}

/// Echo the request's HTTP method name, accepting ANY HTTP method (httpbin #522).
pub async fn method_handler(req: HttpRequest) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(json!({ "method": req.method().as_str() })))
}

/// HEAD-only endpoint echoing request headers as `X-Echo-<name>` response
/// headers, with an empty body (httpbin #630).
pub async fn head_handler(req: HttpRequest) -> Result<HttpResponse> {
    let mut builder = HttpResponse::Ok();
    for (name, value) in req.headers().iter() {
        let Ok(value_str) = value.to_str() else {
            continue;
        };
        if let Ok(echo_name) =
            HeaderName::from_bytes(format!("x-echo-{}", name.as_str()).as_bytes())
        {
            builder.append_header((echo_name, value_str));
        }
    }
    Ok(builder.finish())
}
