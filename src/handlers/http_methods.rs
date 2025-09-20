use super::*;

pub async fn get_handler(req: HttpRequest) -> Result<HttpResponse> {
    let request_info = extract_get_request_info(&req);
    Ok(HttpResponse::Ok().json(request_info))
}

// Universal handler that can handle different content types
pub async fn post_handler(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse> {
    let content_type = req.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.to_lowercase().starts_with("multipart/form-data") {
        // Handle multipart form data
        let multipart = Multipart::new(&req.headers(), payload);
        match extract_request_info_multipart(&req, multipart).await {
            Ok(request_info) => Ok(HttpResponse::Ok().json(request_info)),
            Err(_) => {
                let mut request_info = extract_request_info(&req, None);
                fix_request_info_url(&req, &mut request_info);
                Ok(HttpResponse::Ok().json(request_info))
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
        let mut request_info = extract_request_info(&req, Some(&body_string));
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok().json(request_info))
    }
}

pub async fn put_handler(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse> {
    let content_type = req.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.to_lowercase().starts_with("multipart/form-data") {
        let multipart = Multipart::new(&req.headers(), payload);
        match extract_request_info_multipart(&req, multipart).await {
            Ok(request_info) => Ok(HttpResponse::Ok().json(request_info)),
            Err(_) => {
                let mut request_info = extract_request_info(&req, None);
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
        let mut request_info = extract_request_info(&req, Some(&body_string));
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok().json(request_info))
    }
}

pub async fn patch_handler(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse> {
    let content_type = req.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.to_lowercase().starts_with("multipart/form-data") {
        let multipart = Multipart::new(&req.headers(), payload);
        match extract_request_info_multipart(&req, multipart).await {
            Ok(request_info) => Ok(HttpResponse::Ok().json(request_info)),
            Err(_) => {
                let mut request_info = extract_request_info(&req, None);
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
        let mut request_info = extract_request_info(&req, Some(&body_string));
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok().json(request_info))
    }
}

pub async fn delete_handler(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse> {
    let content_type = req.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.to_lowercase().starts_with("multipart/form-data") {
        let multipart = Multipart::new(&req.headers(), payload);
        match extract_request_info_multipart(&req, multipart).await {
            Ok(request_info) => Ok(HttpResponse::Ok().json(request_info)),
            Err(_) => {
                let mut request_info = extract_request_info(&req, None);
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
        let mut request_info = extract_request_info(&req, Some(&body_string));
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok().json(request_info))
    }
}

