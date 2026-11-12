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

/// The JSON body of /response-headers reports its own Content-Length, so the
/// value must match the actual body size (and the header).
#[actix_web::test]
async fn response_headers_content_length_matches_body() {
    let app = test::init_service(create_app(cfg())).await;
    for uri in [
        "/response-headers",
        "/response-headers?X-Test=bar&X-Test=baz",
        "/response-headers?Content-Type=text/plain",
        "/response-headers?content-type=text/plain",
        "/response-headers?CoNtEnT-TyPe=text/plain",
    ] {
        for method in [actix_web::http::Method::GET, actix_web::http::Method::POST] {
            let req = test::TestRequest::default()
                .method(method.clone())
                .uri(uri)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::OK);
            let header = resp
                .headers()
                .get("content-length")
                .map(|v| v.to_str().unwrap().to_string());
            let body = test::read_body(resp).await;
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let reported = json["Content-Length"].as_str().unwrap().to_string();
            assert_eq!(reported, body.len().to_string(), "{method} {uri}");
            if let Some(header) = header {
                assert_eq!(header, reported, "{method} {uri}");
            }
        }
    }
}

/// Codes outside 100-999 can't be sent as a real status, so they are
/// rejected like any other invalid code instead of falling back to 200.
#[actix_web::test]
async fn status_out_of_range_code_is_bad_request() {
    let app = test::init_service(create_app(cfg())).await;
    for uri in [
        "/status/99",
        "/status/1000",
        "/status/0",
        "/status/200,1000",
    ] {
        for method in [actix_web::http::Method::GET, actix_web::http::Method::POST] {
            let req = test::TestRequest::default()
                .method(method.clone())
                .uri(uri)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{method} {uri}");
        }
    }
}

#[actix_web::test]
async fn response_headers_get_preserves_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    for (uri, expected) in [
        ("/response-headers", "application/json"),
        ("/response-headers?Content-Type=text/plain", "text/plain"),
        ("/response-headers?content-type=text/plain", "text/plain"),
        ("/response-headers?CoNtEnT-TyPe=text/plain", "text/plain"),
    ] {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            expected,
            "{uri}"
        );
    }
}

#[actix_web::test]
async fn response_headers_post_preserves_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    for (uri, expected) in [
        ("/response-headers", "application/json"),
        ("/response-headers?Content-Type=text/plain", "text/plain"),
        ("/response-headers?content-type=text/plain", "text/plain"),
        ("/response-headers?CoNtEnT-TyPe=text/plain", "text/plain"),
    ] {
        let req = test::TestRequest::post().uri(uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            expected,
            "{uri}"
        );
    }
}

/// httpbin #757: /response-headers drops framing, hop-by-hop, Refresh,
/// X-Content-Type-Options and Proxy-* params instead of setting them, both on
/// the wire and in the JSON header dump.
#[actix_web::test]
async fn response_headers_drops_reserved_headers() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri(
            "/response-headers?Transfer-Encoding=chunked&Connection=close&Keep-Alive=timeout%3D5\
             &TE=trailers&Trailer=X-Foo&Upgrade=h2c&Refresh=0&X-Content-Type-Options=nosniff\
             &Proxy-Connection=keep-alive&Proxy-Foo=bar&X-Keep=ok",
        )
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    for name in [
        "transfer-encoding",
        "connection",
        "keep-alive",
        "te",
        "trailer",
        "upgrade",
        "refresh",
        "x-content-type-options",
        "proxy-connection",
        "proxy-foo",
    ] {
        assert!(resp.headers().get(name).is_none(), "{name} must be dropped");
    }
    assert_eq!(
        resp.headers().get("x-keep").unwrap().to_str().unwrap(),
        "ok"
    );
    // Dropped params must not appear in the JSON header dump either.
    let json: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(json["X-Keep"].as_str().unwrap(), "ok");
    assert!(json.get("Refresh").is_none());
    assert!(json.get("Proxy-Foo").is_none());
    assert!(json.get("Transfer-Encoding").is_none());
}

/// httpbin #756: a Content-Type a browser renders or runs is refused with 400
/// (reflected XSS through a crafted link), for GET, POST and the ?body= path.
#[actix_web::test]
async fn response_headers_refuses_renderable_content_types() {
    let app = test::init_service(create_app(cfg())).await;
    for uri in [
        "/response-headers?Content-Type=text/html",
        "/response-headers?content-type=image/svg+xml",
        "/response-headers?Content-Type=application/atom+xml",
        "/response-headers?Content-Type=TEXT%2FHTML%3Bcharset%3Dutf-8",
        "/response-headers?body=%3Cscript%3Ealert(1)%3C%2Fscript%3E&Content-Type=text/html",
        "/response-headers?X-Other=1&Content-Type=application/xhtml+xml",
    ] {
        for method in [actix_web::http::Method::GET, actix_web::http::Method::POST] {
            let req = test::TestRequest::default()
                .method(method.clone())
                .uri(uri)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{method} {uri}");
            assert!(!resp.headers().contains_key("x-other"), "{method} {uri}");
        }
    }
    // Non-renderable types still work.
    let req = test::TestRequest::get()
        .uri("/response-headers?Content-Type=text/plain&X-Ok=1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/plain"
    );
}

/// The ?body= path of /response-headers drops the same reserved headers.
#[actix_web::test]
async fn response_headers_body_path_drops_reserved_headers() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/response-headers?body=hello&Refresh=0&Content-Length=3&Proxy-Connection=x&X-Ok=1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get("refresh").is_none());
    assert!(resp.headers().get("proxy-connection").is_none());
    assert_eq!(resp.headers().get("x-ok").unwrap().to_str().unwrap(), "1");
    // If present, the wire Content-Length must be the real body size, not 3.
    if let Some(len) = resp.headers().get("content-length") {
        assert_eq!(len.to_str().unwrap(), "5");
    }
    assert_eq!(test::read_body(resp).await, "hello");
}

/// /status?header= drops framing and hop-by-hop names (plus Proxy-*), but
/// keeps Refresh and every ordinary header injectable for testing.
#[actix_web::test]
async fn status_header_overrides_drop_framing_and_hop_by_hop() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri(
            "/status/200?header=Content-Length:5&header=Transfer-Encoding:chunked\
             &header=Connection:close&header=Upgrade:h2c&header=Proxy-Connection:x\
             &header=Proxy-Authenticate:Basic&header=Refresh:5&header=X-Ok:yes",
        )
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    for name in [
        "transfer-encoding",
        "connection",
        "upgrade",
        "proxy-connection",
        "proxy-authenticate",
    ] {
        assert!(resp.headers().get(name).is_none(), "{name} must be dropped");
    }
    // If present, the wire Content-Length must be the real body size, not 5.
    if let Some(len) = resp.headers().get("content-length") {
        assert_ne!(len.to_str().unwrap(), "5");
    }
    assert_eq!(
        resp.headers().get("refresh").unwrap().to_str().unwrap(),
        "5"
    );
    assert_eq!(resp.headers().get("x-ok").unwrap().to_str().unwrap(), "yes");
}
