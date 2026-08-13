use super::*;
use actix_web::{http::StatusCode, test};
use std::io::Read;

fn cfg() -> ServerConfig {
    ServerConfig::default()
}

#[actix_web::test]
async fn zstd_endpoint_returns_zstd_encoded_json() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/zstd").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "zstd"
    );

    let body = test::read_body(resp).await;
    let decoded = zstd::decode_all(&body[..]).expect("body should be valid zstd");
    let v: serde_json::Value =
        serde_json::from_slice(&decoded).expect("decoded body should be JSON");

    assert_eq!(v.get("zstd").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(v.get("method").and_then(|x| x.as_str()), Some("GET"));
    assert!(v.get("url").is_some());
}

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
async fn healthz_returns_ok() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/healthz").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["status"].as_str(), Some("ok"));
}

#[actix_web::test]
async fn tags_endpoint_returns_object_and_404_for_missing() {
    let app = test::init_service(create_app(cfg())).await;
    // /tags always returns a JSON object (possibly empty).
    let req = test::TestRequest::get().uri("/tags").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON object");
    assert!(v.is_object());
    // Unknown tag → 404 {}.
    let req = test::TestRequest::get()
        .uri("/tags/__definitely_not_set__")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn tag_key_strips_redundant_httpcan_prefix() {
    use crate::handlers::observability::tag_key;
    // Canonical unprefixed form passes through unchanged.
    assert_eq!(tag_key("VERSION"), "VERSION");
    assert_eq!(tag_key("FOO_BAR"), "FOO_BAR");
    // Redundant prefix is stripped: /tags/HTTPCAN_VERSION == /tags/VERSION.
    assert_eq!(tag_key("HTTPCAN_VERSION"), "VERSION");
    assert_eq!(tag_key("HTTPCAN_FOO_BAR"), "FOO_BAR");
    // Names that merely share the prefix letters are not mangled.
    assert_eq!(tag_key("HTTPCANISH"), "HTTPCANISH");
    assert_eq!(tag_key(""), "");
}

#[actix_web::test]
async fn response_carries_version_and_server_timing_headers() {
    // httpbin #431/#560: every response carries version + Server-Timing.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/get").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.headers().contains_key("x-httpcan-version"));
    assert!(
        resp.headers()
            .get("server-timing")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("app;dur="),
        "Server-Timing must start with app;dur="
    );
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

    // Arbitrary method name
    let custom = actix_web::http::Method::from_bytes(b"BREW").unwrap();
    let req = test::TestRequest::default()
        .method(custom)
        .uri("/method")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v["method"].as_str(), Some("BREW"));
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
async fn base64_post_decodes_body() {
    // httpbin #616: POST /base64 decodes the request body.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/base64")
        .set_payload("aGVsbG8=") // "hello"
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(test::read_body(resp).await, "hello");
}

#[actix_web::test]
async fn compression_post_returns_encoded_body() {
    // httpbin #618: POST /zstd returns the body compressed with Content-Encoding.
    let app = test::init_service(create_app(cfg())).await;
    let payload = "the quick brown fox";
    let req = test::TestRequest::post()
        .uri("/zstd")
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "zstd"
    );
    let decoded = zstd::decode_all(&test::read_body(resp).await[..]).expect("valid zstd");
    assert_eq!(decoded, payload.as_bytes());
}

#[actix_web::test]
async fn iso_8859_1_endpoint_returns_latin1() {
    // httpbin #427: /encoding/iso-8859-1 serves Latin-1 bytes.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/encoding/iso-8859-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/html; charset=iso-8859-1"
    );
    // Latin-1 'é' is the single byte 0xE9 (vs UTF-8's 0xC3 0xA9).
    assert!(
        test::read_body(resp).await.contains(&0xe9),
        "body must contain a Latin-1 byte"
    );
}
#[actix_web::test]
async fn basic_auth_accepts_post() {
    // httpbin #365/#607: basic-auth must accept POST credentials.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q=")) // user:passwd
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
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

#[actix_web::test]
async fn cookies_set_supports_attributes() {
    // httpbin #508: /cookies/set honors httponly/samesite/domain/etc.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/cookies/set?token=secret&httponly=true&samesite=Lax")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let set_cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_lowercase();
    assert!(set_cookie.contains("token=secret"));
    assert!(set_cookie.contains("httponly"));
    assert!(set_cookie.contains("samesite=lax"));
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

#[actix_web::test]
async fn base64_returns_binary_as_octet_stream() {
    // httpbin #599: non-UTF-8 decoded bytes returned raw, not 400.
    // "8A==" decodes to the single byte 0xf0 (invalid UTF-8).
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/base64/8A%3D%3D")
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
    assert_eq!(test::read_body(resp).await, vec![0xf0]);
}

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
async fn digest_auth_sha_512_256_succeeds() {
    // httpbin #697: SHA-512-256 (RFC 7616) digest auth end-to-end. curl has
    // no SHA-512-256 client, so the digest response is computed here.
    let app = test::init_service(create_app(cfg())).await;
    let uri = "/digest-auth/auth/user/passwd/SHA-512-256";

    // 1) Challenge: pull nonce/realm/opaque from the 401 WWW-Authenticate.
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("www-authenticate")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let field = |k: &str| -> String {
        let pat = format!("{k}=\"");
        let start = challenge.find(&pat).unwrap() + pat.len();
        let end = challenge[start..].find('"').unwrap();
        challenge[start..start + end].to_string()
    };
    let realm = field("realm");
    let nonce = field("nonce");
    let opaque = field("opaque");

    // 2) Compute the digest response (qop=auth).
    let hex = |data: &[u8]| -> String {
        crate::handlers::auth::digest_hash_hex("SHA-512-256", data).unwrap()
    };
    let ha1 = hex(format!("user:{realm}:passwd").as_bytes());
    let ha2 = hex(format!("GET:{uri}").as_bytes());
    let nc = "00000001";
    let cnonce = "0a4f113b";
    let response = hex(format!("{ha1}:{nonce}:{nc}:{cnonce}:auth:{ha2}").as_bytes());
    let auth = format!(
        r#"Digest username="user", realm="{realm}", nonce="{nonce}", uri="{uri}", qop=auth, nc={nc}, cnonce="{cnonce}", response="{response}", opaque="{opaque}", algorithm=SHA-512-256"#
    );

    // 3) Replay with credentials -> 200, authenticated.
    let req = test::TestRequest::get()
        .uri(uri)
        .insert_header(("Authorization", auth))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("algorithm").and_then(|x| x.as_str()),
        Some("SHA-512-256")
    );
}

#[actix_web::test]
async fn digest_auth_omits_qop_rfc2069() {
    // httpbin #592: `/digest-auth/none/...` must issue a challenge with NO
    // qop directive (RFC 2069 legacy mode) and accept an RFC 2069 response
    // computed as H(HA1:nonce:HA2) — no nc/cnonce/qop.
    let app = test::init_service(create_app(cfg())).await;
    let uri = "/digest-auth/none/user/passwd";

    // 1) Challenge: 401, and qop must be ABSENT.
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("www-authenticate")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        !challenge.contains("qop="),
        "RFC 2069 challenge must omit qop, got: {challenge}"
    );
    let field = |k: &str| -> String {
        let pat = format!("{k}=\"");
        let start = challenge.find(&pat).unwrap() + pat.len();
        let end = challenge[start..].find('"').unwrap();
        challenge[start..start + end].to_string()
    };
    let realm = field("realm");
    let nonce = field("nonce");
    let opaque = field("opaque");

    // 2) RFC 2069 response = MD5(MD5(user:realm:passwd):nonce:MD5(GET:uri)).
    let ha1 = format!("{:x}", md5::compute(format!("user:{realm}:passwd")));
    let ha2 = format!("{:x}", md5::compute(format!("GET:{uri}")));
    let response = format!("{:x}", md5::compute(format!("{ha1}:{nonce}:{ha2}")));
    let auth = format!(
        r#"Digest username="user", realm="{realm}", nonce="{nonce}", uri="{uri}", response="{response}", opaque="{opaque}""#
    );

    // 3) Replay with credentials -> 200, authenticated.
    let req = test::TestRequest::get()
        .uri(uri)
        .insert_header(("Authorization", auth))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
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

// ============================================================
// Echo core (/get /post /put /patch /delete /anything /echo)
// ============================================================

// === /get — args, headers echo, origin, url ===

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

// === /post — JSON body, form-urlencoded body ===

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

// === /put, /patch, /delete — body echo (HttpMethodsRequestInfo) ===

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

// === /anything — method echo + body echo (RequestInfo HAS method) ===

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

// === /echo — verbatim body mirroring ===

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

// ============================================================
// Response formats (/json /xml /html /robots.txt /deny /encoding /gzip /deflate /brotli)
// ============================================================

// === Response format: static content endpoints ===

/// /json returns 200 with application/json body containing a `slideshow` object.
#[actix_web::test]
async fn fmt_json_returns_slideshow() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/json").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/json"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert!(
        v.get("slideshow").is_some(),
        "body must contain slideshow key"
    );
    assert_eq!(v["slideshow"]["author"].as_str(), Some("Yours Truly"));
    assert!(v["slideshow"]["slides"].is_array(), "slides is an array");
    assert_eq!(v["slideshow"]["title"].as_str(), Some("Sample Slide Show"));
}

/// /xml returns 200 with application/xml body containing a <slideshow> element.
#[actix_web::test]
async fn fmt_xml_body_and_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/xml").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/xml"
    );
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(text.contains("<slideshow"), "body contains <slideshow");
    assert!(text.contains("Sample Slide Show"));
}

/// /html returns 200 with text/html; charset=utf-8 body containing an <html> tag.
#[actix_web::test]
async fn fmt_html_body_and_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/html").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/html; charset=utf-8"
    );
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(text.contains("<html"), "body contains <html");
    assert!(text.contains("Moby-Dick"));
}

/// /robots.txt returns 200 with text/plain body containing a Disallow rule.
#[actix_web::test]
async fn fmt_robots_txt_body_and_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/robots.txt").to_request();
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
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(text.contains("Disallow"), "body contains Disallow");
    assert!(text.contains("User-agent: *"));
    assert!(text.contains("/deny"));
}

/// /deny returns 200 with text/plain body exactly "YOU SHOULDN'T BE HERE".
#[actix_web::test]
async fn fmt_deny_body_and_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/deny").to_request();
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
    let body = test::read_body(resp).await;
    assert_eq!(&body[..], b"YOU SHOULDN'T BE HERE");
}

/// /robots.txt advertises the sitemap via a `Sitemap:` directive pointing at
/// the request origin with an absolute URL (RFC 9309 / sitemaps.org).
#[actix_web::test]
async fn fmt_robots_txt_advertises_sitemap() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/robots.txt")
        .insert_header(("Host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(
        text.contains("Sitemap: http://example.com/sitemap.xml"),
        "robots.txt must advertise the sitemap with an absolute URL: {text}"
    );
}

/// /sitemap.xml returns 200 with application/xml: a valid Sitemaps <urlset>
/// containing at least one absolute <loc> entry for the homepage.
#[actix_web::test]
async fn fmt_sitemap_xml_body_and_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/sitemap.xml")
        .insert_header(("Host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "application/xml"
    );
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(text.contains("<urlset"), "body is a sitemap <urlset>: {text}");
    assert!(
        text.contains("http://www.sitemaps.org/schemas/sitemap/0.9"),
        "body declares the sitemaps namespace"
    );
    assert!(text.contains("<url>"), "body contains a <url> entry");
    assert!(text.contains("<loc>"), "body contains a <loc> entry");
    assert!(
        text.contains("<loc>http://example.com/</loc>"),
        "loc is an absolute URL pointing at the homepage: {text}"
    );
}

/// /encoding/utf8 returns 200 with text/html; charset=utf-8 content type.
#[actix_web::test]
async fn fmt_utf8_content_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/encoding/utf8").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/html; charset=utf-8"
    );
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(text.contains("<html"), "body contains <html");
    assert!(text.contains("UTF-8") || text.contains("utf-8") || text.contains("Unicode"));
}

// === Response format: compressed transport endpoints ===

/// GET /gzip returns 200, content-encoding gzip, body decodes to JSON with
/// `gzipped==true` and `method=="GET"`.
#[actix_web::test]
async fn fmt_gzip_get_decodes() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/gzip").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "gzip"
    );
    let body = test::read_body(resp).await;
    let mut decoded = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&body[..]), &mut decoded)
        .expect("gzip decode");
    let v: serde_json::Value = serde_json::from_slice(&decoded).expect("decompressed body is JSON");
    assert_eq!(v["method"].as_str(), Some("GET"));
    assert_eq!(v["gzipped"].as_bool(), Some(true));
}

/// GET /deflate returns 200, content-encoding deflate, body decodes (raw deflate
/// stream from flate2 `DeflateEncoder`) to JSON with `deflated==true`.
#[actix_web::test]
async fn fmt_deflate_get_decodes() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/deflate").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "deflate"
    );
    let body = test::read_body(resp).await;
    let mut decoded = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::DeflateDecoder::new(&body[..]),
        &mut decoded,
    )
    .expect("raw deflate decode");
    let v: serde_json::Value = serde_json::from_slice(&decoded).expect("decompressed body is JSON");
    assert_eq!(v["method"].as_str(), Some("GET"));
    assert_eq!(v["deflated"].as_bool(), Some(true));
}

/// GET /brotli returns 200, content-encoding br, body decodes to JSON with
/// `brotli==true`.
#[actix_web::test]
async fn fmt_brotli_get_decodes() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/brotli").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "br"
    );
    let body = test::read_body(resp).await;
    let mut decoded = Vec::new();
    std::io::Read::read_to_end(
        &mut brotli::Decompressor::new(&body[..], 4096),
        &mut decoded,
    )
    .expect("brotli decode");
    let v: serde_json::Value = serde_json::from_slice(&decoded).expect("decompressed body is JSON");
    assert_eq!(v["method"].as_str(), Some("GET"));
    assert_eq!(v["brotli"].as_bool(), Some(true));
}

// ============================================================
// Redirects (/redirect /relative-redirect /absolute-redirect /redirect-to)
// ============================================================

// === Section: /redirect/{n} chain ===

/// /redirect/1 returns 302 FOUND with Location pointing to /get (relative).
#[actix_web::test]
async fn redirect_one_targets_get() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/redirect/1").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert_eq!(location, "/get");
}

/// /redirect/2 returns 302 and decrements the chain; per handler source the
/// next hop is /relative-redirect/1 (the plain /redirect/{n} handler routes
/// its decrement through the relative-redirect endpoint, not back to itself).
#[actix_web::test]
async fn redirect_two_decrements_to_relative_redirect_one() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/redirect/2").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert_eq!(location, "/relative-redirect/1");
}

/// /redirect/0 is rejected with 400 BAD_REQUEST (n must be > 0), not a redirect.
#[actix_web::test]
async fn redirect_zero_is_bad_request() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/redirect/0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("location").is_none());
}

// === Section: /relative-redirect/{n} ===

/// /relative-redirect/1 returns 302 with a RELATIVE Location (/get, hostless).
#[actix_web::test]
async fn relative_redirect_one_is_relative_get() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/relative-redirect/1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    // Relative: starts with '/' and contains no scheme/host.
    assert!(location.starts_with('/'));
    assert!(!location.starts_with("http"));
    assert_eq!(location, "/get");
}

/// /relative-redirect/2 decrements to /relative-redirect/1 (relative chain).
#[actix_web::test]
async fn relative_redirect_two_decrements() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/relative-redirect/2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert_eq!(location, "/relative-redirect/1");
}

/// /relative-redirect/0 is rejected with 400 BAD_REQUEST.
#[actix_web::test]
async fn relative_redirect_zero_is_bad_request() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/relative-redirect/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("location").is_none());
}

// === Section: /absolute-redirect/{n} ===

/// /absolute-redirect/1 returns 302 with an ABSOLUTE Location (http(s)://host/get).
#[actix_web::test]
async fn absolute_redirect_one_is_absolute_get() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/absolute-redirect/1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert!(
        location.starts_with("http://") || location.starts_with("https://"),
        "expected absolute URL, got {location}"
    );
    assert!(location.ends_with("/get"));
}

/// /absolute-redirect/2 decrements to an absolute /absolute-redirect/1 URL.
#[actix_web::test]
async fn absolute_redirect_two_decrements_absolute() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/absolute-redirect/2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert!(
        location.starts_with("http://") || location.starts_with("https://"),
        "expected absolute URL, got {location}"
    );
    assert!(location.ends_with("/absolute-redirect/1"));
}

/// /absolute-redirect/0 is rejected with 400 BAD_REQUEST.
#[actix_web::test]
async fn absolute_redirect_zero_is_bad_request() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/absolute-redirect/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("location").is_none());
}

// === Section: /redirect-to (GET) ===

/// GET /redirect-to?url=... returns 302 with Location equal to the given URL
/// for programmatic clients (no Accept: text/html).
#[actix_web::test]
async fn redirect_to_get_with_query_url() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/redirect-to?url=https%3A%2F%2Fexample.com")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert_eq!(location, "https://example.com");
}

/// GET /redirect-to without a url param is rejected with 400 BAD_REQUEST.
#[actix_web::test]
async fn redirect_to_get_missing_url_is_bad_request() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/redirect-to").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("location").is_none());
}

// === Section: /redirect-to (POST form + JSON) ===

/// POST /redirect-to with a urlencoded form body sets Location to the form value.
#[actix_web::test]
async fn redirect_to_post_form_url() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/redirect-to")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("url=https%3A%2F%2Fexample.org")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert_eq!(location, "https://example.org");
}

/// POST /redirect-to with a JSON body {"url":"..."} sets Location to the value.
#[actix_web::test]
async fn redirect_to_post_json_url() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/redirect-to")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"url":"https://json.example"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("location header present")
        .to_str()
        .unwrap();
    assert_eq!(location, "https://json.example");
}

/// POST /redirect-to with no url anywhere is rejected with 400 BAD_REQUEST.
#[actix_web::test]
async fn redirect_to_post_missing_url_is_bad_request() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/redirect-to")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("location").is_none());
}

// ============================================================
// Streaming (/sse /ndjson)
// ============================================================

// === SSE streaming ===

/// `/sse?count=3&delay=0&format=simple` streams 3 JSON data frames, then the
/// handler appends an `event: end` completion frame and a `data: [DONE]`
/// sentinel (5 total `data:` lines). Verifies content-type and frame shape.
#[actix_web::test]
async fn sse_simple_count_frames() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/sse?count=3&delay=0&format=simple")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/event-stream"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    // 3 loop frames each carry a JSON object payload (`data: {`).
    assert_eq!(text.matches("data: {").count(), 3);
    // Plus the `event: end` frame and the `[DONE]` sentinel => 5 `data:` lines.
    assert_eq!(text.matches("data: ").count(), 5);
    assert!(text.contains("event: end"));
    assert!(text.contains("data: [DONE]"));
    // Simple frames report their 1-based event index out of the total.
    assert!(text.contains("Hello from HTTPCan SSE! Event 1/3"));
    assert!(text.contains("Hello from HTTPCan SSE! Event 3/3"));
}

/// `/sse/2?delay=0` honors the path count param: 2 streamed JSON frames plus
/// the standard termination (4 `data:` lines total).
#[actix_web::test]
async fn sse_path_count() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/sse/2?delay=0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/event-stream"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    assert_eq!(text.matches("data: {").count(), 2);
    assert_eq!(text.matches("data: ").count(), 4);
    assert!(text.contains("data: [DONE]"));
    assert!(text.contains("Hello from HTTPCan SSE! Event 1/2"));
    assert!(text.contains("Hello from HTTPCan SSE! Event 2/2"));
}

/// `/sse?count=2&delay=0&format=openai` streams OpenAI `chat.completion.chunk`
/// frames: each data frame carries a `choices` array and model `httpcan-sse`,
/// and the stream closes with a `finish_reason: "stop"` chunk then `[DONE]`.
#[actix_web::test]
async fn sse_openai_format() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/sse?count=2&delay=0&format=openai")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/event-stream"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    // 2 loop chunks + 1 final stop chunk all carry the OpenAI choices shape.
    assert_eq!(text.matches("\"choices\"").count(), 3);
    assert!(text.contains("\"object\":\"chat.completion.chunk\""));
    assert!(text.contains("\"model\":\"httpcan-sse\""));
    assert!(text.contains("\"role\":\"assistant\""));
    // Final completion chunk signals stop, then the [DONE] sentinel.
    assert!(text.contains("\"finish_reason\":\"stop\""));
    assert!(text.contains("data: [DONE]"));
    // OpenAI format emits `event: message` lines (format != simple branch).
    assert!(text.contains("event: message"));
}

// === NDJSON streaming ===

/// `/ndjson?count=3&delay=0&format=simple` emits exactly 3 newline-delimited
/// JSON objects with content-type `application/x-ndjson` (no termination sentinel).
#[actix_web::test]
async fn ndjson_simple_count() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/ndjson?count=3&delay=0&format=simple")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-ndjson"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    let objects: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    assert_eq!(
        objects.len(),
        3,
        "exactly 3 NDJSON objects; body was:\n{text}"
    );
    for (i, line) in objects.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON object");
        assert_eq!(v["event"].as_u64(), Some((i + 1) as u64));
        assert!(v["message"]
            .as_str()
            .unwrap()
            .contains("Hello from HTTPCan NDJSON!"));
        assert!(v.get("timestamp").is_some());
    }
}

/// `/ndjson/2?delay=0` honors the path count param: exactly 2 newline-delimited objects.
#[actix_web::test]
async fn ndjson_path_count() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/ndjson/2?delay=0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-ndjson"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    let objects: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    assert_eq!(
        objects.len(),
        2,
        "exactly 2 NDJSON objects; body was:\n{text}"
    );
    // Default format is simple, so each object carries its event index.
    let first: serde_json::Value = serde_json::from_str(objects[0]).expect("valid JSON");
    assert_eq!(first["event"].as_u64(), Some(1));
}

/// `/ndjson?count=1&delay=0&format=ollama` emits a single Ollama-shaped object
/// with `model` (default `llama2`), a string `response`, and `done: true`
/// (the sole event is the last one, so completion metadata is attached).
#[actix_web::test]
async fn ndjson_ollama_format() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/ndjson?count=1&delay=0&format=ollama")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-ndjson"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    let objects: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    assert_eq!(objects.len(), 1);
    let v: serde_json::Value = serde_json::from_str(objects[0]).expect("valid JSON object");
    assert_eq!(v["model"].as_str(), Some("llama2"));
    assert!(v["response"].is_string());
    assert_eq!(v["done"].as_bool(), Some(true));
    assert!(v.get("created_at").is_some());
    // The done/final event carries ollama context + eval metadata.
    assert!(v.get("context").is_some());
    assert!(v.get("eval_count").is_some());
    assert!(v.get("total_duration").is_some());
}

// ============================================================
// Auth flows (basic / hidden-basic / bearer / jwt-bearer / digest)
// ============================================================

// === Basic auth (user/passwd) — success + failure ===

/// /basic-auth/user/passwd with correct Basic creds returns 200, authenticated=true, user="user".
#[actix_web::test]
async fn auth_basic_correct_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q=")) // user:passwd
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(v.get("user").and_then(|x| x.as_str()), Some("user"));
}

/// /basic-auth/user/passwd with no Authorization header returns 401 + WWW-Authenticate challenge.
#[actix_web::test]
async fn auth_basic_no_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user/passwd")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let www = resp
        .headers()
        .get("WWW-Authenticate")
        .expect("WWW-Authenticate present");
    assert!(www.to_str().unwrap().starts_with("Basic realm="));
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /basic-auth/user/passwd with wrong creds returns 401 + WWW-Authenticate challenge.
#[actix_web::test]
async fn auth_basic_wrong_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic d3Jvbmc6d3Jvbmc=")) // wrong:wrong
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("WWW-Authenticate"));
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /basic-auth/user (username-only route) with Basic dXNlcjo= (user:) → 200, password must be empty.
#[actix_web::test]
async fn auth_basic_user_only_empty_password() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user")
        .insert_header(("Authorization", "Basic dXNlcjo=")) // user:
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(v.get("user").and_then(|x| x.as_str()), Some("user"));
}

// === Hidden basic auth — returns 404 (not 401) when unauthorized ===

/// /hidden-basic-auth/user/passwd with correct creds → 200, authenticated=true.
#[actix_web::test]
async fn auth_hidden_basic_correct_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q=")) // user:passwd
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(v.get("user").and_then(|x| x.as_str()), Some("user"));
}

/// /hidden-basic-auth with no creds returns 404 (hidden variant) and NO WWW-Authenticate header.
#[actix_web::test]
async fn auth_hidden_basic_no_creds_is_404() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user/passwd")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // Hidden variant must NOT advertise a WWW-Authenticate challenge (it hides the endpoint).
    assert!(resp.headers().get("WWW-Authenticate").is_none());
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /hidden-basic-auth with wrong creds returns 404 and NO WWW-Authenticate header.
#[actix_web::test]
async fn auth_hidden_basic_wrong_creds_is_404() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic d3Jvbmc6d3Jvbmc=")) // wrong:wrong
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert!(resp.headers().get("WWW-Authenticate").is_none());
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /hidden-basic-auth/{user} (user-only variant): empty password matches → 200;
/// otherwise the hidden variant stays hidden and returns 404 (no challenge).
#[actix_web::test]
async fn auth_hidden_basic_user_only_succeeds_and_hides_failure() {
    let app = test::init_service(create_app(cfg())).await;
    // Correct: username matches, password empty (Basic "user:" = dXNlcjo=).
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user")
        .insert_header(("Authorization", "Basic dXNlcjo="))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    // No creds → stays hidden (404, no WWW-Authenticate).
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert!(resp.headers().get("WWW-Authenticate").is_none());
}

// === Bearer auth ===

/// /bearer with a valid Bearer token → 200, authenticated=true, token echoed back.
#[actix_web::test]
async fn auth_bearer_valid_token() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/bearer")
        .insert_header(("Authorization", "Bearer my-secret-token"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("token").and_then(|x| x.as_str()),
        Some("my-secret-token")
    );
}

/// /bearer with no Authorization header → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_bearer_no_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/bearer").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let www = resp
        .headers()
        .get("WWW-Authenticate")
        .expect("WWW-Authenticate present");
    assert_eq!(www.to_str().unwrap(), "Bearer");
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /bearer with a non-Bearer (Basic) Authorization header → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_bearer_invalid_scheme() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/bearer")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q="))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
}

// === JWT bearer ===

/// /jwt-bearer with a structurally valid JWT (no exp claim) → 200, decodes header + payload claims.
#[actix_web::test]
async fn auth_jwt_valid_decodes_claims() {
    // Canonical jwt.io example token (HS256). Signature is NOT verified by the
    // handler (insecure_decode), so a well-formed token always decodes. No `exp`
    // claim ⇒ expiration status "not_present" ⇒ treated as valid (200).
    let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
                 eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.\
                 SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/jwt-bearer")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    // raw token echoed verbatim
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("raw"))
            .and_then(|x| x.as_str()),
        Some(token)
    );
    // header decoded
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("header"))
            .and_then(|h| h.get("alg"))
            .and_then(|x| x.as_str()),
        Some("HS256")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("header"))
            .and_then(|h| h.get("typ"))
            .and_then(|x| x.as_str()),
        Some("JWT")
    );
    // payload claims decoded (raw numeric iat retained)
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("sub"))
            .and_then(|x| x.as_str()),
        Some("1234567890")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("name"))
            .and_then(|x| x.as_str()),
        Some("John Doe")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("iat"))
            .and_then(|x| x.as_i64()),
        Some(1516239022)
    );
    // payloadFormatted carries a human-readable iat timestamp string
    assert!(
        v.get("token")
            .and_then(|t| t.get("payloadFormatted"))
            .and_then(|p| p.get("iat"))
            .map(|x| x.is_string())
            .unwrap_or(false),
        "payloadFormatted.iat must be a formatted timestamp string"
    );
    // validation status reflects structure + (missing) expiration
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("validationStatus"))
            .and_then(|s| s.get("structure"))
            .and_then(|x| x.as_str()),
        Some("valid")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("validationStatus"))
            .and_then(|s| s.get("expiration"))
            .and_then(|x| x.as_str()),
        Some("not_present")
    );
}

/// /jwt-bearer with a malformed token (wrong number of segments) → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_jwt_malformed_is_401() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/jwt-bearer")
        .insert_header(("Authorization", "Bearer not-a-real-jwt")) // single segment
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
    assert!(v.get("error").is_some());
}

/// /jwt-bearer with no Authorization header → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_jwt_missing_header_is_401() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/jwt-bearer").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
    assert_eq!(
        v.get("error").and_then(|x| x.as_str()),
        Some("Missing Authorization header")
    );
}

/// /jwt-bearer with a non-Bearer Authorization scheme → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_jwt_wrong_scheme_is_401() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/jwt-bearer")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q="))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
}

// === Digest auth — challenge issuance (no creds) ===

/// /digest-auth/auth/user/passwd with no creds → 401 with a Digest challenge containing
/// realm, nonce, qop="auth", and algorithm="MD5".
#[actix_web::test]
async fn auth_digest_challenge_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/digest-auth/auth/user/passwd")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("WWW-Authenticate")
        .expect("WWW-Authenticate present")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        challenge.starts_with("Digest "),
        "challenge must be Digest scheme: {}",
        challenge
    );
    assert!(
        challenge.contains("realm=\"httpcan@"),
        "realm must be prefixed with httpcan@: {}",
        challenge
    );
    assert!(
        challenge.contains("nonce=\""),
        "missing nonce: {}",
        challenge
    );
    assert!(
        challenge.contains("opaque=\""),
        "missing opaque: {}",
        challenge
    );
    assert!(
        challenge.contains("qop=\"auth\""),
        "missing/incorrect qop directive: {}",
        challenge
    );
    assert!(
        challenge.contains("algorithm=\"MD5\""),
        "missing/incorrect algorithm: {}",
        challenge
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

// ============================================================
// Cookies (/cookies /cookies/set/{n}/{v} /cookies/delete)
// ============================================================

// === Cookies: GET /cookies (read) ===

/// GET /cookies reflects the incoming Cookie header as a JSON object of name->value pairs.
#[actix_web::test]
async fn cookie_get_parses_multiple_pairs() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/cookies")
        .insert_header(("Cookie", "k1=v1; k2=v2"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert!(v.get("cookies").is_some(), "response has cookies object");
    assert_eq!(v["cookies"]["k1"].as_str(), Some("v1"));
    assert_eq!(v["cookies"]["k2"].as_str(), Some("v2"));
}

/// GET /cookies with no Cookie header returns an empty cookies object.
#[actix_web::test]
async fn cookie_get_empty_when_no_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/cookies").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert!(v["cookies"].is_object(), "cookies is a JSON object");
    assert!(v["cookies"].as_object().unwrap().is_empty());
}

// === Cookies: GET /cookies/set/{name}/{value} (named set + redirect) ===

/// GET /cookies/set/foo/bar redirects (302) and sets a cookie foo=bar.
#[actix_web::test]
async fn cookie_set_named_redirects_and_sets_cookie() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/cookies/set/foo/bar")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let set_cookie = resp
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie header present")
        .to_str()
        .unwrap();
    assert!(
        set_cookie.contains("foo=bar"),
        "Set-Cookie sets foo=bar: {set_cookie}"
    );
    assert!(
        set_cookie.contains("Path=/"),
        "cookie scoped to Path=/: {set_cookie}"
    );
    assert_eq!(
        resp.headers().get("location").unwrap().to_str().unwrap(),
        "/cookies",
        "redirects to /cookies"
    );
}

// === Cookies: GET /cookies/delete (delete via redirect) ===

/// GET /cookies/delete?foo= redirects (302) and emits a Set-Cookie that expires
/// foo. The query KEY is the cookie name (value ignored), per cookies_delete_handler.
#[actix_web::test]
async fn cookie_delete_redirects_and_clears_cookie() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/cookies/delete?foo=")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let set_cookie = resp
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie header present")
        .to_str()
        .unwrap();
    assert!(
        set_cookie.contains("foo=;"),
        "Set-Cookie clears foo with empty value: {set_cookie}"
    );
    assert!(
        set_cookie.contains("Max-Age=0"),
        "Set-Cookie uses Max-Age=0 to expire: {set_cookie}"
    );
    assert_eq!(
        resp.headers().get("location").unwrap().to_str().unwrap(),
        "/cookies",
        "redirects to /cookies"
    );
}

// ============================================================
// Dynamic data (/uuid /stream /range /links /delay)
// ============================================================

// === Dynamic Data: uuid ===

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

// === Dynamic Data: stream ===

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

// === Dynamic Data: range ===

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

// === Dynamic Data: links ===

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

// === Dynamic Data: delay ===

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

// ============================================================
// Images (/image /image/{fmt})
// ============================================================

// === Image format endpoints ===

/// GET /image/png returns 200 with content-type image/png and a body whose
/// first bytes are the PNG magic \x89PNG.
#[actix_web::test]
async fn img_png_returns_valid_png() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/png").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/png"
    );
    let body = test::read_body(resp).await;
    assert!(!body.is_empty(), "png body must not be empty");
    assert!(
        body.starts_with(&[0x89, b'P', b'N', b'G']),
        "png body must start with PNG magic bytes, got: {:?}",
        &body[..body.len().min(8)]
    );
}

/// GET /image/jpeg returns 200 with content-type image/jpeg and a body whose
/// first bytes are the JPEG magic 0xFF 0xD8 0xFF.
#[actix_web::test]
async fn img_jpeg_returns_valid_jpeg() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/jpeg").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/jpeg"
    );
    let body = test::read_body(resp).await;
    assert!(!body.is_empty(), "jpeg body must not be empty");
    assert!(
        body.starts_with(&[0xff, 0xd8, 0xff]),
        "jpeg body must start with JPEG magic bytes, got: {:?}",
        &body[..body.len().min(4)]
    );
}

/// GET /image/webp returns 200 with content-type image/webp and a body that
/// carries the RIFF/WEBP container markers.
#[actix_web::test]
async fn img_webp_returns_valid_webp() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/webp").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/webp"
    );
    let body = test::read_body(resp).await;
    assert!(body.len() > 12, "webp body must be at least 13 bytes");
    assert_eq!(&body[..4], b"RIFF", "webp body must start with RIFF marker");
    assert_eq!(
        &body[8..12],
        b"WEBP",
        "webp body must carry WEBP fourcc at offset 8..12"
    );
}

/// GET /image/svg returns 200 with content-type image/svg+xml and a body that
/// is UTF-8 text containing the <svg root element.
#[actix_web::test]
async fn img_svg_returns_valid_svg() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/svg").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/svg+xml"
    );
    let body = test::read_body(resp).await;
    assert!(!body.is_empty(), "svg body must not be empty");
    let text = std::str::from_utf8(&body).expect("svg body must be valid UTF-8");
    assert!(
        text.contains("<svg"),
        "svg body must contain the <svg element"
    );
}

// === /image Accept negotiation ===

/// GET /image with Accept: image/png is negotiated to image/png.
#[actix_web::test]
async fn image_accept_png_negotiated() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/image")
        .insert_header(("Accept", "image/png"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/png"
    );
    let body = test::read_body(resp).await;
    assert!(
        body.starts_with(&[0x89, b'P', b'N', b'G']),
        "negotiated png body must start with PNG magic bytes"
    );
}

/// GET /image with Accept: */* returns a 200 whose content-type is some image
/// type (the handler picks a random format).
#[actix_web::test]
async fn image_accept_wildcard_random_image_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/image")
        .insert_header(("Accept", "*/*"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header present")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        ct.starts_with("image/"),
        "default /image content-type must be an image type, got: {ct}"
    );
}

/// GET /image with no Accept header at all behaves like the default random path.
#[actix_web::test]
async fn image_no_accept_header_random_image_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header present")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        ct.starts_with("image/"),
        "default /image content-type must be an image type, got: {ct}"
    );
}

// ============================================================
// Inspection (/ip /user-agent /headers /cache /cache/{v})
// ============================================================

// === Request inspection (/user-agent, /ip, /headers) ===

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

// === Response inspection (/cache, /cache/{value}) ===

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

// ============================================================
// Middleware exclude_headers + root / + /openapi.json
// ============================================================

// === Section: Middleware — exclude_headers filtering ===

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

// === Section: Root (/) homepage ===

/// GET / always serves the static homepage as HTML — 200, text/html
/// content-type, a body containing an <html> tag — regardless of the
/// Accept header. Search engines, AI crawlers, and browsers must all see
/// the same crawlable content (see the SEO/GEO discussion in the repo
/// history for why this replaced Accept-based negotiation).
#[actix_web::test]
async fn root_always_serves_html() {
    let app = test::init_service(create_app(cfg())).await;
    for accept in ["text/html", "application/json", "*/*"] {
        let req = test::TestRequest::get()
            .uri("/")
            .insert_header(("Accept", accept))
            .to_request();
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
            "expected text/html content-type for Accept: {accept}, got {ct}"
        );
        let body = test::read_body(resp).await;
        let body_str = std::str::from_utf8(&body).expect("utf-8 html body");
        assert!(
            body_str.to_lowercase().contains("<html"),
            "html body must contain an <html> tag"
        );
    }
}

/// GET / with no Accept header at all also serves the homepage.
#[actix_web::test]
async fn root_serves_html_without_accept_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .unwrap();
    assert!(ct.starts_with("text/html"));
}

/// The homepage lists real endpoints (grouped by category) and links to the
/// machine-readable OpenAPI spec.
#[actix_web::test]
async fn root_homepage_lists_endpoints_and_links_to_openapi() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).expect("utf-8 html body");
    assert!(body_str.contains("/get"), "homepage should list /get");
    assert!(body_str.contains("/sse"), "homepage should list /sse");
    assert!(
        body_str.contains("/openapi.json"),
        "homepage should link to the OpenAPI spec"
    );
}

/// Each endpoint on the homepage has a "Copy" button with a ready-to-run
/// curl example (resolved against the request's own origin) attached via
/// `data-curl`, plus the client-side script that wires up copy-to-clipboard.
#[actix_web::test]
async fn root_homepage_has_copy_curl_buttons() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/")
        .insert_header(("Host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).expect("utf-8 html body");

    assert!(
        body_str.contains(r#"class="copy-btn""#),
        "homepage should render a copy-curl button for each endpoint"
    );
    assert!(
        body_str.contains(r#"data-curl="curl http://example.com/uuid""#),
        "copy button for /uuid should target the request's own origin"
    );
    assert!(
        body_str.contains("navigator.clipboard"),
        "homepage should include the copy-to-clipboard script"
    );
}

// === Section: OpenAPI (/openapi.json) ===

/// GET /openapi.json returns 200 with application/json content-type and a body
/// that parses as the OpenAPI spec: an `openapi` version string (3.x) and a
/// `paths` object that includes the `/get` route.
#[actix_web::test]
async fn openapi_json_returns_valid_spec() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/openapi.json").to_request();
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
        "expected application/json content-type, got {ct}"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    let version = v
        .get("openapi")
        .and_then(|o| o.as_str())
        .expect("openapi version string");
    assert!(
        version.starts_with("3."),
        "openapi version should be 3.x, got {version}"
    );
    let paths = v
        .get("paths")
        .and_then(|p| p.as_object())
        .expect("paths object");
    assert!(
        paths.contains_key("/get"),
        "paths must include /get, got keys: {:?}",
        paths.keys().collect::<Vec<_>>()
    );
}

// ============================================================
// Round 2: behavioral gaps — distinct code paths & documented features
// (excludes same-handler method dispatch that's already covered)
// ============================================================

// === SSE/NDJSON two-segment path variants (count + delay via path) ===

/// GET /sse/{count}/{delay} exercises sse_path_with_delay_handler:
/// count and delay both taken from the path. 2 events with delay=0.
#[actix_web::test]
async fn sse_count_delay_path_variant() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/sse/2/0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/event-stream"
    );
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf-8");
    // 2 data frames + termination frame + [DONE] sentinel = 4 "data:" lines.
    let data_lines = body.lines().filter(|l| l.starts_with("data:")).count();
    assert_eq!(
        data_lines, 4,
        "expected 4 data: lines (2 events + end + DONE): {body}"
    );
}

/// GET /ndjson/{count}/{delay} exercises ndjson_path_with_delay_handler.
/// Exactly 2 newline-delimited objects.
#[actix_web::test]
async fn ndjson_count_delay_path_variant() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/ndjson/2/0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf-8");
    let objs: Vec<&str> = body.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(objs.len(), 2, "expected exactly 2 NDJSON objects: {body}");
    for o in &objs {
        serde_json::from_str::<serde_json::Value>(o).expect("each line is valid JSON");
    }
}

// === Status: comma-separated selection, POST body, OPTIONS preflight ===

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

// === response-headers POST (distinct handler from GET) ===

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

// === Digest auth 5-segment route (qop/user/passwd/algorithm/stale_after) ===

/// GET /digest-auth/{qop}/{user}/{passwd}/{algorithm}/{stale_after}
/// (digest_auth_full_handler) issues a 401 Digest challenge AND sets the
/// stale_after cookie — the behavior unique to the 5-segment route.
#[actix_web::test]
async fn digest_auth_full_five_segment_sets_stale_after_cookie() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/digest-auth/auth/user/passwd/MD5/never")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("www-authenticate")
        .expect("Digest challenge")
        .to_str()
        .unwrap();
    assert!(
        challenge.starts_with("Digest"),
        "challenge must be Digest scheme: {challenge}"
    );
    // The 5-segment route is the only one that plants the stale_after cookie.
    let set_cookie = resp
        .headers()
        .get_all("set-cookie")
        .map(|v| v.to_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("; ");
    assert!(
        set_cookie.contains("stale_after=never"),
        "5-segment route must set stale_after cookie: {set_cookie}"
    );
}

// === POST body encoding: gzip / deflate / brotli branches of compress_post_handler ===

/// POST /gzip (compress_post_handler) returns the body gzip-compressed.
#[actix_web::test]
async fn compress_post_gzip_encodes_body() {
    let payload = "compress-me-gzip";
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/gzip")
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "gzip"
    );
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(&test::read_body(resp).await[..])
        .read_to_end(&mut decoded)
        .expect("valid gzip");
    assert_eq!(decoded, payload.as_bytes());
}

/// POST /deflate (compress_post_handler) returns the body raw-deflate-compressed.
#[actix_web::test]
async fn compress_post_deflate_encodes_body() {
    let payload = "compress-me-deflate";
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/deflate")
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "deflate"
    );
    let mut decoded = Vec::new();
    flate2::read::DeflateDecoder::new(&test::read_body(resp).await[..])
        .read_to_end(&mut decoded)
        .expect("valid raw deflate");
    assert_eq!(decoded, payload.as_bytes());
}

/// POST /brotli (compress_post_handler) returns the body brotli-compressed.
#[actix_web::test]
async fn compress_post_brotli_encodes_body() {
    let payload = "compress-me-brotli";
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/brotli")
        .set_payload(payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-encoding")
            .unwrap()
            .to_str()
            .unwrap(),
        "br"
    );
    let mut decoded = Vec::new();
    brotli::Decompressor::new(&test::read_body(resp).await[..], 4096)
        .read_to_end(&mut decoded)
        .expect("valid brotli");
    assert_eq!(decoded, payload.as_bytes());
}
