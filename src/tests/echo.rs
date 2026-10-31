//! Echo core and HTTP methods: /get /post /put /patch /delete /anything /echo, QUERY/OPTIONS/TRACE/HEAD, multipart

use super::*;

#[actix_web::test]
async fn query_method_anything_echoes_method_and_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(query_method())
        .uri("/anything")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"hello":"world"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("method").and_then(|x| x.as_str()), Some("QUERY"));
    assert_eq!(v["json"]["hello"].as_str(), Some("world"));
    assert!(v.get("url").is_some());
}

#[actix_web::test]
async fn query_method_anything_with_param_records_path() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(query_method())
        .uri("/anything/foo/bar")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("method").and_then(|x| x.as_str()), Some("QUERY"));
    assert_eq!(v["args"]["anything"].as_str(), Some("foo/bar"));
}

#[actix_web::test]
async fn query_method_echo_mirrors_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(query_method())
        .uri("/echo")
        .set_payload("ping")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(test::read_body(resp).await, "ping");
}

// Dedicated echo endpoints completing the standard-method family:
// /options (RFC 9110 §9.3.7), /trace (§9.8), /query (RFC 9430).
#[actix_web::test]
async fn options_endpoint_echoes_request_with_allow_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/options?k=v")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    // RFC 9110 §10.2.1: Allow lists the target resource's methods.
    let allow = resp
        .headers()
        .get("allow")
        .and_then(|v| v.to_str().ok())
        .expect("Allow header on OPTIONS success");
    assert!(allow.contains("OPTIONS"), "Allow: {allow}");

    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["args"]["k"].as_str(), Some("v"));
    assert!(v.get("url").is_some());
}

#[actix_web::test]
async fn trace_endpoint_echoes_method() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::TRACE)
        .uri("/trace")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("method").and_then(|x| x.as_str()), Some("TRACE"));
    assert!(v.get("url").is_some());
}

#[actix_web::test]
async fn query_endpoint_parses_url_args_and_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(query_method())
        .uri("/query?lang=rust")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("select=stars")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    // Both channels echoed: URL args and the body-carried query (RFC 9430:
    // the body *is* the query).
    assert_eq!(v["args"]["lang"].as_str(), Some("rust"));
    assert_eq!(v["form"]["select"].as_str(), Some("stars"));
    assert!(v.get("url").is_some());
}

#[actix_web::test]
async fn dedicated_method_endpoints_405_on_wrong_method() {
    // Each dedicated endpoint answers only its own method with 200;
    // mismatches are 405 + Allow, like the rest of the method family.
    let app = test::init_service(create_app(cfg())).await;
    for (uri, allowed) in [
        ("/options", "OPTIONS"),
        ("/trace", "TRACE"),
        ("/query", "QUERY"),
    ] {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED, "GET {uri}");
        let allow = resp
            .headers()
            .get("allow")
            .and_then(|v| v.to_str().ok())
            .expect("Allow header on 405");
        assert!(allow.contains(allowed), "Allow: {allow}");
    }
}

#[actix_web::test]
async fn duplicate_request_headers_are_joined() {
    // Regression for httpbin #355: multiple headers with the same name
    // must be joined with ", " instead of collapsing to the last value.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/headers")
        .append_header(("x-multi", "Foo"))
        .append_header(("x-multi", "Bar"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v["headers"]["x-multi"].as_str(),
        Some("Foo, Bar"),
        "duplicate header values must be joined per RFC 7230"
    );
}

#[actix_web::test]
async fn multipart_parses_json_fields_and_captures_file_content_type() {
    // httpbin #693/#722: JSON form parts must be parsed into objects, and
    // file parts must capture their per-part Content-Type.
    let boundary = "----httpcan-test";
    let body = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"obj\"\r\n\
         Content-Type: application/json\r\n\r\n\
         {{\"k\":\"v\"}}\r\n\
         --{b}\r\n\
         Content-Disposition: form-data; name=\"f\"; filename=\"a.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         hello\r\n\
         --{b}--\r\n",
        b = boundary
    );
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/post")
        .insert_header((
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");

    // #693: JSON part parsed into an object, not an escaped string.
    assert_eq!(v["form"]["obj"]["k"].as_str(), Some("v"));

    // #722: file part is an object carrying content_type + filename + content.
    assert_eq!(v["files"]["f"]["content_type"].as_str(), Some("text/plain"));
    assert_eq!(v["files"]["f"]["filename"].as_str(), Some("a.txt"));
    assert_eq!(v["files"]["f"]["content"].as_str(), Some("hello"));
}

#[actix_web::test]
async fn method_endpoint_echoes_arbitrary_method() {
    // httpbin #522: /method accepts ANY method name and echoes it.
    let app = test::init_service(create_app(cfg())).await;

    // Standard method
    let req = test::TestRequest::get().uri("/method").to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("GET"));

    // Arbitrary method names, including the spec's additionalOperations
    // placeholder key ANY_CUSTOM_METHOD (self-demonstrating: the echoed
    // response teaches the mechanism).
    for name in ["BREW", "ANY_CUSTOM_METHOD"] {
        let custom = actix_web::http::Method::from_bytes(name.as_bytes()).unwrap();
        let req = test::TestRequest::default()
            .method(custom)
            .uri("/method")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK, "method {name}");
        let v: serde_json::Value =
            serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
        assert_eq!(v["method"].as_str(), Some(name));
    }
}

#[actix_web::test]
async fn head_endpoint_echoes_request_headers() {
    // httpbin #630: HEAD /head echoes request headers as X-Echo-*, empty body.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::HEAD)
        .uri("/head")
        .insert_header(("X-Test", "hello"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        test::read_body(resp).await.is_empty(),
        "HEAD response must have an empty body"
    );
}

#[actix_web::test]
async fn head_requests_match_get_routes() {
    // Drop-in parity with httpbin (RFC 9110 §9.3.2): Flask auto-serves HEAD
    // on GET routes, so every GET endpoint must answer HEAD with the GET
    // status/headers; the HTTP codec drops the body at the wire level.
    let app = test::init_service(create_app(cfg())).await;
    for uri in [
        "/get",
        "/echo",
        "/anything",
        "/headers",
        "/ip",
        "/json",
        "/xml",
        "/uuid",
        "/base64/aGVsbG8=",
        "/encoding/utf8",
        "/robots.txt",
        "/sitemap.xml",
        "/healthz",
        "/tags",
        "/sse",
        "/ndjson",
    ] {
        let req = test::TestRequest::default()
            .method(actix_web::http::Method::HEAD)
            .uri(uri)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "HEAD {uri} must serve the GET route"
        );
    }
}

#[actix_web::test]
async fn head_redirects_and_auth_keep_semantics() {
    // HEAD preserves non-200 semantics: redirects keep Location, auth
    // challenges keep WWW-Authenticate, hidden auth stays 404.
    let app = test::init_service(create_app(cfg())).await;

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::HEAD)
        .uri("/redirect/2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        resp.headers().get("location").and_then(|v| v.to_str().ok()),
        Some("/relative-redirect/1")
    );

    let req = test::TestRequest::default()
        .method(actix_web::http::Method::HEAD)
        .uri("/bearer")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

#[actix_web::test]
async fn head_on_non_get_endpoints_is_405() {
    // Flask does not auto-map HEAD onto POST-only routes: httpbin answers
    // 405 there (never 404). Method endpoints must keep that behavior.
    let app = test::init_service(create_app(cfg())).await;
    for uri in [
        "/post", "/put", "/patch", "/delete", "/options", "/trace", "/query", "/base64",
    ] {
        let req = test::TestRequest::default()
            .method(actix_web::http::Method::HEAD)
            .uri(uri)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "HEAD {uri} must be 405, not 404"
        );
    }

    // 405 carries an Allow header listing the served methods (httpbin/Flask parity).
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::HEAD)
        .uri("/post")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let allow = resp
        .headers()
        .get("allow")
        .and_then(|v| v.to_str().ok())
        .expect("Allow header on 405");
    assert!(allow.contains("POST"), "Allow: {allow}");

    // Method mismatch on a GET endpoint is 405 with GET/HEAD allowed.
    let req = test::TestRequest::post().uri("/get").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    let allow = resp
        .headers()
        .get("allow")
        .and_then(|v| v.to_str().ok())
        .expect("Allow header on 405");
    assert!(
        allow.contains("GET") && allow.contains("HEAD"),
        "Allow: {allow}"
    );
}

/// /get reflects query params in `args`, echoes a custom header in `headers`,
/// and populates `origin` and `url`. (GetRequestInfo has no `method`/`data`.)
#[actix_web::test]
async fn echo_get_args_headers_origin_url() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/get?foo=bar")
        .insert_header(("x-echo-core", "sentinel"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");

    // args reflect the query string (single value → JSON string, not array)
    assert_eq!(v["args"]["foo"].as_str(), Some("bar"));

    // custom request header is echoed back (header names are lowercased)
    assert_eq!(v["headers"]["x-echo-core"].as_str(), Some("sentinel"));

    // origin and url are populated
    let origin = v["origin"].as_str().expect("origin is a string");
    assert!(!origin.is_empty(), "origin must not be empty");
    let url = v["url"].as_str().expect("url is a string");
    assert!(url.contains("/get"), "url should reference /get: {url}");
    assert!(url.contains("foo=bar"), "url should carry the query: {url}");

    // GetRequestInfo has no method/data field — confirm absence so a future
    // schema regression (accidentally adding method) is caught.
    assert!(v.get("method").is_none(), "/get must not expose method");
    assert!(v.get("data").is_none(), "/get must not expose data");
}

/// /post with an application/json body parses it into `json` and keeps the raw
/// bytes in `data`. (HttpMethodsRequestInfo has no `method` field.)
#[actix_web::test]
async fn echo_post_json_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/post")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"k":"v"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");

    assert_eq!(
        v["json"]["k"].as_str(),
        Some("v"),
        "json.k should be parsed"
    );
    // raw body is preserved verbatim in `data`
    assert_eq!(v["data"].as_str(), Some(r#"{"k":"v"}"#));
    // supporting fields present
    assert!(v.get("url").is_some());
    let origin = v["origin"].as_str().expect("origin is a string");
    assert!(!origin.is_empty());
    assert!(
        v.get("method").is_none(),
        "/post must not expose method (HttpMethodsRequestInfo)"
    );
}

/// /post with application/x-www-form-urlencoded parses fields into `form` and
/// leaves the raw body in `data`.
#[actix_web::test]
async fn echo_post_form_urlencoded() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/post")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("a=1&b=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");

    assert_eq!(v["form"]["a"].as_str(), Some("1"));
    assert_eq!(v["form"]["b"].as_str(), Some("2"));
    // form-urlencoded bodies are routed into `form`; `data` stays empty (per extract_request_info)
    assert_eq!(v["data"].as_str(), Some(""));
    // form content is not JSON-parsed
    assert!(v["json"].is_null(), "form body must not populate json");
}

/// /put accepts a PUT request and echoes the raw body in `data`.
#[actix_web::test]
async fn echo_put_echoes_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::put()
        .uri("/put")
        .set_payload("put-payload")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["data"].as_str(), Some("put-payload"));
    assert!(v.get("url").is_some());
}

/// /patch accepts a PATCH request and echoes the raw body in `data`.
#[actix_web::test]
async fn echo_patch_echoes_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::patch()
        .uri("/patch")
        .set_payload("patch-payload")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["data"].as_str(), Some("patch-payload"));
    assert!(v.get("url").is_some());
}

/// /delete accepts a DELETE request and echoes the raw body in `data`.
#[actix_web::test]
async fn echo_delete_echoes_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::delete()
        .uri("/delete")
        .set_payload("delete-payload")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["data"].as_str(), Some("delete-payload"));
    assert!(v.get("url").is_some());
}

/// /anything GET returns the full RequestInfo including `method` == "GET".
#[actix_web::test]
async fn echo_anything_get_method() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/anything").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("GET"));
    assert!(v.get("url").is_some());
    let origin = v["origin"].as_str().expect("origin is a string");
    assert!(!origin.is_empty());
}

/// /anything POST with a JSON body echoes `method` == "POST", parses `json`,
/// and preserves the raw body in `data`.
#[actix_web::test]
async fn echo_anything_post_echoes_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/anything")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"k":"v"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("POST"));
    assert_eq!(v["json"]["k"].as_str(), Some("v"));
    assert_eq!(v["data"].as_str(), Some(r#"{"k":"v"}"#));
}

/// /anything/{path:.*} captures the entire multi-segment tail under the
/// `anything` key in `args` (confirmed in anything.rs / process_request_payload).
#[actix_web::test]
async fn echo_anything_path_param_multisegment() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/anything/foo/bar")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["args"]["anything"].as_str(), Some("foo/bar"));
    assert_eq!(v["method"].as_str(), Some("GET"));
}

/// GET /echo returns 200 with an empty body (no request body to mirror).
#[actix_web::test]
async fn echo_get_returns_empty_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/echo").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(body.is_empty(), "GET /echo body must be empty");
}

/// POST /echo returns the exact request body bytes verbatim.
#[actix_web::test]
async fn echo_post_body_verbatim() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/echo")
        .set_payload("hello")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert_eq!(&body[..], b"hello", "POST /echo must mirror body bytes");
}

/// PUT, PATCH, and DELETE on /echo each mirror the request body verbatim.
#[actix_web::test]
async fn echo_put_patch_delete_body_verbatim() {
    let app = test::init_service(create_app(cfg())).await;
    let cases: &[(Method, &str)] = &[
        (Method::PUT, "put-echo"),
        (Method::PATCH, "patch-echo"),
        (Method::DELETE, "delete-echo"),
    ];
    for (method, payload) in cases {
        let req = test::TestRequest::default()
            .method(method.clone())
            .uri("/echo")
            .set_payload(*payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK, "status for {method:?}");
        let body = test::read_body(resp).await;
        assert_eq!(
            &body[..],
            payload.as_bytes(),
            "{method:?} /echo must mirror body bytes verbatim"
        );
    }
}

/// /body is the facet-named primary for /echo: POST returns the body verbatim,
/// and /echo remains a permanent compatibility alias with identical behavior.
#[actix_web::test]
async fn body_post_verbatim_and_echo_alias_equivalent() {
    let app = test::init_service(create_app(cfg())).await;
    for uri in ["/body", "/echo"] {
        let req = test::TestRequest::post()
            .uri(uri)
            .insert_header(("Content-Type", "text/plain"))
            .set_payload("mirror me")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK, "POST {uri}");
        let body = test::read_body(resp).await;
        assert_eq!(&body[..], b"mirror me", "POST {uri} must mirror body bytes");
    }
}
