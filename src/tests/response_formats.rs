//! Response formats and compression: /json /xml /html /robots.txt /deny /encoding /gzip /deflate /brotli /zstd /base64

use super::*;

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

#[actix_web::test]
async fn fmt_robots_txt_honors_canonical_scheme() {
    let app = test::init_service(create_app(
        cfg().canonical_scheme(config::SchemeOverride::Https),
    ))
    .await;
    let req = test::TestRequest::get()
        .uri("/robots.txt")
        .insert_header(("Host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(
        text.contains("Sitemap: https://example.com/sitemap.xml"),
        "canonical_scheme(Https) must force an https sitemap URL even for a plain-http test request: {text}"
    );
}

/// /robots.txt declares AI usage preferences via Content-Signal directives
/// (https://contentsignals.org/): search allowed, training and AI input denied.
/// The directive must sit inside the `User-agent: *` group — before the
/// groupless `Sitemap:` line — so it applies to every crawler.
#[actix_web::test]
async fn fmt_robots_txt_declares_content_signals() {
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
        text.contains("Content-Signal: ai-train=yes, search=yes, ai-input=yes"),
        "robots.txt must declare Content-Signal preferences: {text}"
    );
    // Structural placement: inside the User-agent group, before the groupless Sitemap line.
    let ua = text.find("User-agent: *").unwrap();
    let cs = text.find("Content-Signal:").unwrap();
    let sm = text.find("Sitemap:").unwrap();
    assert!(
        ua < cs,
        "Content-Signal must follow the User-agent group header"
    );
    assert!(
        cs < sm,
        "Content-Signal must be inside the User-agent group, before the groupless Sitemap directive"
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
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/xml"
    );
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(
        text.contains("<urlset"),
        "body is a sitemap <urlset>: {text}"
    );
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

#[actix_web::test]
async fn fmt_sitemap_xml_honors_canonical_scheme() {
    let app = test::init_service(create_app(
        cfg().canonical_scheme(config::SchemeOverride::Https),
    ))
    .await;
    let req = test::TestRequest::get()
        .uri("/sitemap.xml")
        .insert_header(("Host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let text = std::str::from_utf8(&body).expect("utf8 body");
    assert!(
        text.contains("<loc>https://example.com/</loc>"),
        "canonical_scheme(Https) must force an https loc even for a plain-http test request: {text}"
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
