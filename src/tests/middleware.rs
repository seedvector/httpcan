//! Middleware: exclude_headers filtering, CORS, version/Server-Timing headers, access-log sizing

use super::*;

#[actix_web::test]
async fn response_carries_version_and_server_timing_headers() {
    // httpbin #431/#560: every response carries version + Server-Timing.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/get").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.headers().contains_key("x-httpcan-version"));
    let timing = resp
        .headers()
        .get("server-timing")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        timing.starts_with("app;dur="),
        "Server-Timing must start with app;dur="
    );
    // Duration is logged with 3 decimal places (microsecond-scale handler
    // times); the old 2-place format truncated every sub-10us request to
    // duration_ms=0.00.
    let value = timing.strip_prefix("app;dur=").unwrap();
    let (_, frac) = value
        .split_once('.')
        .expect("Server-Timing duration must contain a decimal point");
    assert!(
        !frac.is_empty() && frac.len() <= 3 && frac.chars().all(|c| c.is_ascii_digit()),
        "Server-Timing duration must be milliseconds with 3 decimal places, got {value}"
    );
}

#[actix_web::test]
async fn response_size_bytes_reads_body_size_not_content_length() {
    // Regression: the access log used to
    // read the Content-Length response header, which actix-http sets only
    // while writing to the socket - after the logger middleware - so
    // size_bytes was always 0. The body's own size is available at
    // middleware time.
    let req = test::TestRequest::get().uri("/get").to_http_request();

    // Sized body: reports the exact byte length even without a
    // Content-Length header in the response map.
    let body = r#"{"args":{},"origin":"127.0.0.1"}"#;
    let resp = actix_web::HttpResponse::Ok()
        .content_type("application/json")
        .body(body);
    assert!(resp.headers().get("content-length").is_none());
    let res = actix_web::dev::ServiceResponse::new(req.clone(), resp);
    assert_eq!(
        crate::middleware::response_size_bytes(&res),
        body.len() as u64
    );

    // Empty (None-sized) body: 0, never a panic or phantom length.
    let resp = actix_web::HttpResponse::Ok().finish();
    let res = actix_web::dev::ServiceResponse::new(req, resp);
    assert_eq!(crate::middleware::response_size_bytes(&res), 0);
}

#[actix_web::test]
async fn cors_exposes_www_authenticate() {
    // httpbin #641: cross-origin clients must be able to read the 401
    // challenge, so CORS exposes WWW-Authenticate.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/bearer")
        .insert_header(("Origin", "https://example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().get("www-authenticate").is_some());
    let expose = resp
        .headers()
        .get("access-control-expose-headers")
        .expect("expose-headers present on cross-origin response")
        .to_str()
        .unwrap();
    assert!(
        expose.to_lowercase().contains("www-authenticate"),
        "WWW-Authenticate must be listed as exposed: {expose}"
    );
}

/// exclude_header("x-secret") removes that header from /headers responses,
/// while a non-excluded header passes through unchanged.
#[actix_web::test]
async fn mw_exclude_header_removes_named_header() {
    let app = test::init_service(create_app(
        ServerConfig::default().exclude_header("x-secret"),
    ))
    .await;
    let req = test::TestRequest::get()
        .uri("/headers")
        .insert_header(("X-Secret", "hidden"))
        .insert_header(("X-Other", "v"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    let headers = v
        .get("headers")
        .and_then(|h| h.as_object())
        .expect("headers object");
    assert!(
        headers.get("x-secret").is_none(),
        "excluded x-secret must not appear in headers"
    );
    assert_eq!(
        headers.get("x-other").and_then(|v| v.as_str()),
        Some("v"),
        "non-excluded x-other must be present"
    );
}

/// exclude_header("x-prod-*") uses wildcard-suffix matching: any header whose
/// lowercased name starts with "x-prod-" is dropped, while "x-dev-token" survives.
#[actix_web::test]
async fn mw_exclude_header_wildcard_suffix() {
    let app = test::init_service(create_app(
        ServerConfig::default().exclude_header("x-prod-*"),
    ))
    .await;
    let req = test::TestRequest::get()
        .uri("/get")
        .insert_header(("X-Prod-Token", "t"))
        .insert_header(("X-Dev-Token", "d"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    let headers = v
        .get("headers")
        .and_then(|h| h.as_object())
        .expect("headers object");
    assert!(
        headers.get("x-prod-token").is_none(),
        "wildcard-matched x-prod-token must not appear in headers"
    );
    assert_eq!(
        headers.get("x-dev-token").and_then(|v| v.as_str()),
        Some("d"),
        "x-dev-token is outside the wildcard prefix and must remain"
    );
}
