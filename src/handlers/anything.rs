use super::*;


pub async fn anything_handler_get(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_handler(req: HttpRequest, payload: web::Payload, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let content_type = req.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.to_lowercase().starts_with("multipart/form-data") {
        let multipart = Multipart::new(&req.headers(), payload);
        match extract_request_info_multipart(&req, multipart, &config.exclude_headers).await {
            Ok(mut request_info) => {
                fix_request_info_url(&req, &mut request_info);
                Ok(HttpResponse::Ok().json(request_info))
            }
            Err(_) => {
                let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
                fix_request_info_url(&req, &mut request_info);
                Ok(HttpResponse::Ok().json(request_info))
            }
        }
    } else {
        use actix_web::web::BytesMut;
        use futures_util::StreamExt;
        
        let mut body = BytesMut::new();
        let mut payload = payload;
        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            body.extend_from_slice(&chunk);
        }
        
        let body_string = String::from_utf8_lossy(&body);
        let mut request_info = extract_request_info(&req, Some(&body_string), &config.exclude_headers);
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok().json(request_info))
    }
}

pub async fn anything_with_param_handler_get(
    req: HttpRequest,
    path: web::Path<String>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    // Add the path parameter to the response (use "anything" as key for consistency)
    request_info.args.insert("anything".to_string(), path.into_inner());
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn anything_with_param_handler(
    req: HttpRequest,
    path: web::Path<String>,
    payload: web::Payload,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let content_type = req.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.to_lowercase().starts_with("multipart/form-data") {
        let multipart = Multipart::new(&req.headers(), payload);
        match extract_request_info_multipart(&req, multipart, &config.exclude_headers).await {
            Ok(mut request_info) => {
                fix_request_info_url(&req, &mut request_info);
                // Add the path parameter to the response (use "anything" as key for consistency)
                request_info.args.insert("anything".to_string(), path.into_inner());
                Ok(HttpResponse::Ok().json(request_info))
            }
            Err(_) => {
                let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
                fix_request_info_url(&req, &mut request_info);
                // Add the path parameter to the response (use "anything" as key for consistency)
                request_info.args.insert("anything".to_string(), path.into_inner());
                Ok(HttpResponse::Ok().json(request_info))
            }
        }
    } else {
        use actix_web::web::BytesMut;
        use futures_util::StreamExt;
        
        let mut body = BytesMut::new();
        let mut payload = payload;
        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            body.extend_from_slice(&chunk);
        }
        
        let body_string = String::from_utf8_lossy(&body);
        let mut request_info = extract_request_info(&req, Some(&body_string), &config.exclude_headers);
        fix_request_info_url(&req, &mut request_info);
        // Add the path parameter to the response (use "anything" as key for consistency)
        request_info.args.insert("anything".to_string(), path.into_inner());
        Ok(HttpResponse::Ok().json(request_info))
    }
}

