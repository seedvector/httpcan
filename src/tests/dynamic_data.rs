//! Dynamic data: /uuid /bytes /stream-bytes /stream /range /drip /links /delay

use super::*;

#[actix_web::test]
async fn drip_chunked_streams_unknown_length_body() {
    // httpbin #479: ?chunked=true streams with chunked transfer-encoding
    // (unknown-length body -> codec emits no Content-Length); the default
    // SizedStream path reports a known length (-> Content-Length).
    use actix_web::body::{BodySize, MessageBody};
    let app = test::init_service(create_app(cfg())).await;

    // Chunked: body size is unknown, content still arrives intact.
    let req = test::TestRequest::get()
        .uri("/drip?numbytes=5&duration=0&delay=0&chunked=true")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        matches!(resp.response().body().size(), BodySize::Stream),
        "chunked response must be an unknown-length stream body"
    );
    assert_eq!(test::read_body(resp).await, "*****");

    // Default (non-chunked): SizedStream reports the exact byte count.
    let req = test::TestRequest::get()
        .uri("/drip?numbytes=5&duration=0&delay=0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        matches!(resp.response().body().size(), BodySize::Sized(5)),
        "non-chunked response must be a sized body of numbytes"
    );
    assert_eq!(test::read_body(resp).await, "*****");
}

#[actix_web::test]
async fn bytes_returns_requested_bytes_under_limit() {
    // httpbin #594: under-limit requests serve exactly n bytes.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/bytes/16?seed=1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/octet-stream"
    );
    assert_eq!(test::read_body(resp).await.len(), 16);
}

#[actix_web::test]
async fn bytes_rejects_over_limit_with_404() {
    // httpbin #594: over-limit requests must NOT be silently truncated;
    // return 404 like /range/{numbytes}.
    let limit = crate::config::DEFAULT_MAX_BYTES;
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/bytes/{}", limit + 1))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body_bytes = test::read_body(resp).await;
    let body = std::str::from_utf8(&body_bytes).unwrap();
    assert!(
        body.contains(&format!("(0, {}]", limit)),
        "error must state the range, got: {body}"
    );
}

#[actix_web::test]
async fn stream_bytes_rejects_over_limit_with_404() {
    // httpbin #594: /stream-bytes must behave consistently with /bytes.
    let limit = crate::config::DEFAULT_MAX_BYTES;
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/stream-bytes/{}", limit + 1))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn bytes_honors_custom_max_bytes() {
    // The limit is configurable via ServerConfig::max_bytes (httpbin #594).
    let custom = ServerConfig::default().max_bytes(10);
    let app = test::init_service(create_app(custom)).await;
    // Over the custom limit -> 404.
    let req = test::TestRequest::get().uri("/bytes/20").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // Within the custom limit -> exactly n bytes.
    let req = test::TestRequest::get().uri("/bytes/5?seed=2").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(test::read_body(resp).await.len(), 5);
}

/// GET /uuid returns 200 with a JSON object whose `uuid` field is a well-formed v4 UUID string.
#[actix_web::test]
async fn dyn_uuid_returns_valid_v4_uuid() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/uuid").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    let uuid_str = v["uuid"].as_str().expect("uuid field is a string");
    // v4 UUID: 36 chars total, 5 groups separated by 4 dashes.
    assert_eq!(uuid_str.len(), 36, "uuid must be 36 chars");
    assert_eq!(
        uuid_str.chars().filter(|c| *c == '-').count(),
        4,
        "uuid must contain 4 dashes"
    );
    // Every char is a hex digit or dash.
    assert!(
        uuid_str.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
        "uuid must be hex+dashes only"
    );
    // Version nibble (char at index 14) must be '4' for v4.
    assert_eq!(
        uuid_str.as_bytes()[14],
        b'4',
        "uuid version nibble must be 4"
    );
    // Variant nibble (index 19) must be 8, 9, a, or b.
    let variant = uuid_str.as_bytes()[19] as char;
    assert!(
        matches!(variant, '8' | '9' | 'a' | 'b'),
        "uuid variant nibble must be one of 8/9/a/b, got {variant}"
    );
}

/// GET /stream/3 returns 200 with content-type application/json and 3 newline-delimited JSON
/// objects each carrying url/args/headers/origin and a sequential id (0..3).
#[actix_web::test]
async fn dyn_stream_ndjson_objects() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/stream/3").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .unwrap();
    assert!(
        ct.starts_with("application/json"),
        "stream content-type should be application/json, got {ct}"
    );

    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    // Each object is terminated by '\n'; trailing newline yields an empty final segment.
    let objects: Vec<&str> = text.lines().collect();
    assert_eq!(objects.len(), 3, "expected 3 ndjson objects");

    for (idx, line) in objects.iter().enumerate() {
        let obj: serde_json::Value = serde_json::from_str(line).expect("each line is JSON");
        assert_eq!(obj["id"].as_u64(), Some(idx as u64), "sequential id");
        assert!(obj.get("url").is_some(), "url field present");
        assert!(obj.get("args").is_some(), "args field present");
        assert!(obj.get("headers").is_some(), "headers field present");
        assert!(obj.get("origin").is_some(), "origin field present");
    }
}

/// GET /range/1024 with no Range header returns 200 (full content) of exactly 1024 bytes with
/// content-type application/octet-stream and Accept-Ranges: bytes.
#[actix_web::test]
async fn dyn_range_full_content_200() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/range/1024").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .unwrap();
    assert!(
        ct.starts_with("application/octet-stream"),
        "range content-type should be application/octet-stream, got {ct}"
    );
    assert_eq!(
        resp.headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok()),
        Some("bytes")
    );
    let body = test::read_body(resp).await;
    assert_eq!(body.len(), 1024, "full range body must be 1024 bytes");
    // Deterministic payload: byte i is b'a' + (i % 26).
    assert_eq!(body[0], b'a');
    assert_eq!(body[25], b'z');
    assert_eq!(body[26], b'a');
}

/// GET /range/1024 with `Range: bytes=10-19` returns 206 Partial Content with a 10-byte body
/// and a matching Content-Range header.
#[actix_web::test]
async fn dyn_range_partial_206() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/range/1024")
        .insert_header(("Range", "bytes=10-19"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok()),
        Some("bytes 10-19/1024")
    );
    assert_eq!(
        resp.headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok()),
        Some("10")
    );
    let body = test::read_body(resp).await;
    assert_eq!(body.len(), 10, "partial body must be 10 bytes");
    // Deterministic payload for positions 10..=19: b'a' + (i % 26).
    for (offset, byte) in body.iter().enumerate() {
        let i = 10 + offset;
        assert_eq!(*byte, b'a' + (i % 26) as u8, "byte at position {i}");
    }
}

/// GET /links/2 redirects (302 Found) to the first link page /links/2/0.
#[actix_web::test]
async fn dyn_links_redirect_302() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/links/2").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        resp.headers().get("location").and_then(|v| v.to_str().ok()),
        Some("/links/2/0")
    );
}

/// GET /links/2/0 returns 200 HTML with 2 entries; the entry at the offset (0) is plain text
/// and the other (1) is an anchor pointing back into /links/2/<i>.
#[actix_web::test]
async fn dyn_links_page_html_entries() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/links/2/0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .unwrap();
    assert!(
        ct.starts_with("text/html"),
        "links content-type should be text/html, got {ct}"
    );

    let body = test::read_body(resp).await;
    let html = std::str::from_utf8(&body).expect("utf8 html");
    assert!(html.contains("<!DOCTYPE html>"), "html doctype");
    // Offset entry (i==0) is rendered as plain text, not a link.
    assert!(html.contains("0<br>"), "offset entry rendered as text");
    // The non-offset entry (i==1) is an anchor.
    assert!(
        html.contains("<a href='/links/2/1'>1</a><br>"),
        "non-offset entry rendered as anchor"
    );
    // Exactly one anchor: offset consumes one of the two entries.
    let anchor_count = html.matches("<a ").count();
    assert_eq!(anchor_count, 1, "only the non-offset entry is a link");
}

/// GET /delay/0 returns 200 (near-instant) echoing the request like /get, including method and url.
#[actix_web::test]
async fn dyn_delay_get_echoes_request() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/delay/0?x=1")
        .insert_header(("x-test-echo", "yes"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("GET"));
    assert!(
        v["url"].as_str().unwrap_or("").contains("/delay/0"),
        "url echoed: {}",
        v["url"]
    );
    assert_eq!(v["args"]["x"].as_str(), Some("1"), "query args echoed");
    assert!(v.get("headers").is_some(), "headers echoed");
    assert!(v.get("origin").is_some(), "origin echoed");
    assert_eq!(v["data"].as_str(), Some(""), "no body data for GET");
}

/// POST /delay/0 returns 200 with method POST and the request body surfaced in `data`.
#[actix_web::test]
async fn dyn_delay_post_echoes_body() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/delay/0")
        .insert_header(("content-type", "text/plain"))
        .set_payload("hello-dyn")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("POST"));
    assert_eq!(v["data"].as_str(), Some("hello-dyn"), "body echoed in data");
    assert!(
        v["url"].as_str().unwrap_or("").contains("/delay/0"),
        "url echoed"
    );
}

/// Negative / non-finite wait parameters are rejected with 400 (finite
/// values above 10s are clamped, like /delay always did).
/// Previously they reached `Duration::from_secs_f64`, which panics — and
/// with `panic = "abort"` in release, one such request killed the server.
#[actix_web::test]
async fn invalid_wait_seconds_rejected_with_400() {
    let app = test::init_service(create_app(cfg())).await;
    for uri in [
        "/delay/-1",
        "/delay/NaN",
        "/delay/inf",
        "/drip?duration=-1&numbytes=5",
        "/drip?duration=NaN&numbytes=5",
        "/drip?delay=-1&numbytes=5",
        "/range/10?duration=-1",
        "/range/10?duration=NaN",
    ] {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{uri}");
    }

    let req = test::TestRequest::post().uri("/delay/-1").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "POST /delay/-1");
}

#[actix_web::test]
async fn range_honors_custom_max_bytes() {
    // /range used to hardcode a 102400 cap regardless of --max-bytes.
    let custom = ServerConfig::default().max_bytes(10);
    let app = test::init_service(create_app(custom)).await;

    let req = test::TestRequest::get().uri("/range/20").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "over-limit /range must 404, not silently cap"
    );

    let req = test::TestRequest::get().uri("/range/10").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn drip_honors_custom_max_bytes() {
    // /drip used to silently clamp numbytes to 10MB; it must follow the same
    // configurable cap as /bytes, /stream-bytes, and /range.
    let custom = ServerConfig::default().max_bytes(10);
    let app = test::init_service(create_app(custom)).await;

    let req = test::TestRequest::get()
        .uri("/drip?numbytes=20&duration=0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "over-limit /drip numbytes must 404, not silently clamp"
    );

    let req = test::TestRequest::get()
        .uri("/drip?numbytes=5&duration=0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
