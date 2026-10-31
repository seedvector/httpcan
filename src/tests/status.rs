//! Status codes and /response-headers

use super::*;

#[actix_web::test]
async fn status_injects_repeatable_response_headers() {
    // httpbin #413/#579: /status must support repeatable ?header=Name:Value
    // injection (e.g. 429 + Retry-After and X-RateLimit-* headers).
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/status/429?header=Retry-After:60&header=X-RateLimit-Remaining:0")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        resp.headers().get("retry-after").unwrap().to_str().unwrap(),
        "60"
    );
    assert_eq!(
        resp.headers()
            .get("x-ratelimit-remaining")
            .unwrap()
            .to_str()
            .unwrap(),
        "0"
    );
}

#[actix_web::test]
async fn status_ignores_trailing_path() {
    // httpbin #714: /status/{codes}/extra must not 404.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/status/200/extra/path")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn response_headers_custom_body() {
    // httpbin #655: ?body=<text> returns a custom body; other params are headers.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/response-headers?body=hello&X-Test=bar")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("x-test").unwrap().to_str().unwrap(),
        "bar"
    );
    assert_eq!(test::read_body(resp).await, "hello");
}

/// /status/{codes} with comma-separated codes (parse_weighted_codes) picks one
/// of the listed codes (documented README feature).
#[actix_web::test]
async fn status_comma_separated_picks_a_listed_code() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/status/200,418").to_request();
    let resp = test::call_service(&app, req).await;
    let code = resp.status().as_u16();
    assert!(
        matches!(code, 200 | 418),
        "must pick one of the listed codes, got {code}"
    );
}

/// POST /status/{codes} (status_handler) uses the request body as the response
/// body when present (distinct from the GET query-only path).
#[actix_web::test]
async fn status_post_echoes_request_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/status/200")
        .set_payload("hello-body")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(test::read_body(resp).await, "hello-body");
}

/// OPTIONS /status/{codes} (status_options_handler) returns 200 with CORS
/// preflight headers.
#[actix_web::test]
async fn status_options_returns_cors_preflight() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/status/200")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let allow = resp
        .headers()
        .get("access-control-allow-methods")
        .expect("CORS allow-methods header")
        .to_str()
        .unwrap();
    assert!(
        allow.contains("GET") && allow.contains("POST"),
        "preflight must list allowed methods: {allow}"
    );
    assert!(resp.headers().contains_key("access-control-max-age"));
}

/// POST /response-headers (response_headers_post_handler) sets response headers
/// from query params, same contract as GET.
#[actix_web::test]
async fn response_headers_post_sets_headers() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/response-headers?X-Post-Test=bar")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("x-post-test").unwrap().to_str().unwrap(),
        "bar"
    );
}
