//! Request/response inspection: /ip /user-agent /headers /cache /etag

use super::*;

#[actix_web::test]
async fn etag_weak_match_and_multisegment_route() {
    // httpbin #400: weak ETag in If-None-Match matches (RFC 7232), and a
    // weak ETag value (with '/') routes instead of 404.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/etag/abc")
        .insert_header(("If-None-Match", "W/\"abc\""))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    // Multi-segment etag value routes (no 404) — supports weak ETags like W/"abc".
    let req = test::TestRequest::get()
        .uri("/etag/W/%22abc%22")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// GET /user-agent echoes the request's User-Agent header as JSON `{"user-agent": ...}`.
#[actix_web::test]
async fn inspect_user_agent() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/user-agent")
        .insert_header(("User-Agent", "TestAgent/1.0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["user-agent"].as_str(), Some("TestAgent/1.0"));
}

/// GET /ip returns a JSON object with a non-empty `origin` string field.
#[actix_web::test]
async fn inspect_ip_origin_present() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/ip").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    let origin = v["origin"]
        .as_str()
        .unwrap_or_else(|| panic!("origin not a string: {:?}", v["origin"]));
    assert!(!origin.is_empty(), "origin should be a non-empty string");
}

/// GET /headers echoes a single custom header under its lowercased name and
/// includes a standard header (host) in the `headers` object.
#[actix_web::test]
async fn inspect_headers_custom_and_standard() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/headers")
        .insert_header(("X-Foo", "bar"))
        .insert_header(("host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    let headers = v["headers"]
        .as_object()
        .expect("headers should be an object");
    // Custom header is echoed under its lowercased name.
    assert_eq!(headers.get("x-foo").and_then(|h| h.as_str()), Some("bar"));
    // A standard header is present.
    assert_eq!(
        headers.get("host").and_then(|h| h.as_str()),
        Some("example.com")
    );
}

/// GET /cache with no conditional headers returns 200 with a request-info body
/// and Last-Modified / ETag cache headers.
#[actix_web::test]
async fn cache_no_conditional_returns_200_with_cache_headers() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/cache").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // Cache-related response headers are present and non-empty.
    let last_modified = resp
        .headers()
        .get("last-modified")
        .expect("Last-Modified header present");
    assert!(!last_modified.is_empty());
    let etag = resp.headers().get("etag").expect("ETag header present");
    assert!(!etag.is_empty());
    // Body echoes request info.
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("GET"));
    assert!(v["headers"].is_object(), "headers object present");
    assert!(v["url"].is_string(), "url field present");
    assert!(v["origin"].is_string(), "origin field present");
}

/// GET /cache with an If-Modified-Since header returns 304 Not Modified. The
/// handler treats the mere presence of If-Modified-Since as a cache hit and
/// emits Last-Modified / ETag headers on the 304.
#[actix_web::test]
async fn cache_if_modified_since_returns_304() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/cache")
        .insert_header(("If-Modified-Since", "Wed, 21 Oct 2015 07:28:00 GMT"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    assert!(resp.headers().contains_key("last-modified"));
    assert!(resp.headers().contains_key("etag"));
}

/// GET /cache with an If-None-Match header also triggers the 304 branch.
#[actix_web::test]
async fn cache_if_none_match_returns_304() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/cache")
        .insert_header(("If-None-Match", "\"abc123\""))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    assert!(resp.headers().contains_key("last-modified"));
    assert!(resp.headers().contains_key("etag"));
}

/// GET /cache/{value} returns 200 with a Cache-Control header of the form
/// `public, max-age={value}` and a request-info body.
#[actix_web::test]
async fn cache_control_max_age_60() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/cache/60").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "public, max-age=60"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("GET"));
    assert!(v["headers"].is_object());
}

/// GET /cache/{value} parameterizes the max-age; value 0 yields `public, max-age=0`.
#[actix_web::test]
async fn cache_control_max_age_0() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/cache/0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "public, max-age=0"
    );
}
