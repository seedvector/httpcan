//! Static asset overrides: disk file wins, embedded/dynamic default otherwise

use super::*;

/// Create a unique temp static dir so each test controls exactly which
/// override files exist (the default `cfg()` resolves to the repo's own
// `static/`, which is not hermetic).
fn temp_static_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "httpcan-test-static-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp static dir");
    dir
}

fn cfg_with_static(dir: &std::path::Path) -> ServerConfig {
    ServerConfig {
        static_dir: Some(dir.to_path_buf()),
        ..ServerConfig::default()
    }
}

fn write_static(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write override file");
}

#[actix_web::test]
async fn openapi_json_served_from_embedded_spec_when_no_override() {
    let dir = temp_static_dir("openapi-embedded");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.starts_with("application/json"));
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("valid JSON spec");
    assert!(
        v["openapi"].as_str().unwrap().starts_with("3."),
        "embedded spec should be OpenAPI 3.x"
    );
    assert!(
        v["paths"].get("/get").is_some(),
        "embedded spec should document /get"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn openapi_disk_override_wins_over_embedded() {
    let dir = temp_static_dir("openapi-override");
    write_static(
        &dir,
        "openapi.json",
        r#"{"info":{"title":"MyCustomCan"},"paths":{}}"#,
    );
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("valid JSON");
    assert_eq!(v["info"]["title"].as_str(), Some("MyCustomCan"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn openapi_no_current_server_serves_base_servers_verbatim() {
    // Embedded copy, injection disabled: only the spec's own servers.
    let dir = temp_static_dir("openapi-ncs-embedded");
    let cfg = ServerConfig {
        add_current_server: false,
        static_dir: Some(dir.clone()),
        ..ServerConfig::default()
    };
    let app = test::init_service(create_app(cfg)).await;
    let req = test::TestRequest::get().uri("/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("valid JSON");
    assert_eq!(v["servers"].as_array().map(Vec::len), Some(1));
    assert_eq!(v["servers"][0]["url"].as_str(), Some("https://httpcan.org"));

    // Override copy, injection disabled: user's servers verbatim.
    write_static(
        &dir,
        "openapi.json",
        r#"{"servers":[{"url":"https://mine.example"}],"paths":{}}"#,
    );
    let cfg = ServerConfig {
        add_current_server: false,
        static_dir: Some(dir.clone()),
        ..ServerConfig::default()
    };
    let app = test::init_service(create_app(cfg)).await;
    let req = test::TestRequest::get().uri("/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("valid JSON");
    assert_eq!(v["servers"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        v["servers"][0]["url"].as_str(),
        Some("https://mine.example")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn openapi_servers_do_not_leak_across_requests() {
    // Regression: the embedded spec is a shared LazyLock; mutating it in
    // place would leak one request's Host into every other response and
    // grow the servers array without bound.
    let dir = temp_static_dir("openapi-leak");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;

    let req = test::TestRequest::get()
        .uri("/openapi.json")
        .insert_header(("host", "first.test"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v1: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("valid JSON");
    let servers1 = v1["servers"].as_array().expect("servers array");
    assert_eq!(servers1.len(), 2, "current + spec server, nothing more");
    assert!(servers1[0]["url"].as_str().unwrap().contains("first.test"));

    let req = test::TestRequest::get()
        .uri("/openapi.json")
        .insert_header(("host", "second.test"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v2: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("valid JSON");
    let servers2 = v2["servers"].as_array().expect("servers array");
    assert_eq!(servers2.len(), 2, "no accumulation across requests");
    assert!(servers2[0]["url"].as_str().unwrap().contains("second.test"));
    let body_str = serde_json::to_string(&v2).unwrap();
    assert!(
        !body_str.contains("first.test"),
        "previous request's origin must not leak into later responses"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn openapi_spec_version_matches_crate_version() {
    let dir = temp_static_dir("openapi-version");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("valid JSON");
    assert_eq!(
        v["info"]["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "embedded spec must be bumped together with Cargo.toml"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn favicon_served_from_embedded_bytes_by_default() {
    let dir = temp_static_dir("favicon-embedded");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/favicon.png").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.starts_with("image/png"));
    let body = test::read_body(resp).await;
    assert!(body.len() > 100, "embedded favicon should be a real PNG");
    assert_eq!(&body[..4], b"\x89PNG", "PNG magic bytes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn favicon_disk_override_wins() {
    let dir = temp_static_dir("favicon-override");
    write_static(&dir, "favicon.png", "NOT-A-REAL-PNG-FAVICON");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/favicon.png").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(test::read_body(resp).await, "NOT-A-REAL-PNG-FAVICON");
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn robots_txt_disk_override_served_as_text_plain() {
    let dir = temp_static_dir("robots-override");
    write_static(&dir, "robots.txt", "User-agent: *\nDisallow: /\n");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/robots.txt").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.starts_with("text/plain"));
    let body = test::read_body(resp).await;
    assert_eq!(body, "User-agent: *\nDisallow: /\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn sitemap_disk_override_served_as_application_xml() {
    let dir = temp_static_dir("sitemap-override");
    write_static(&dir, "sitemap.xml", "<urlset>USER-SITEMAP</urlset>");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/sitemap.xml").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/xml",
        "override must match the dynamic handler's content type, not mime_guess' text/xml"
    );
    assert_eq!(test::read_body(resp).await, "<urlset>USER-SITEMAP</urlset>");
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn homepage_index_html_override_replaces_homepage() {
    let dir = temp_static_dir("index-override");
    write_static(
        &dir,
        "index.html",
        "<!DOCTYPE html><html><body><h1>MyCan</h1></body></html>",
    );
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.starts_with("text/html"));
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("<h1>MyCan</h1>"));
    assert!(
        !body.contains("HTTPCan"),
        "built-in homepage must be fully replaced"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn api_routes_shadow_same_named_static_files() {
    let dir = temp_static_dir("shadow");
    write_static(&dir, "json", "SHADOWED-FILE-CONTENT");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get().uri("/json").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(
        body.contains("slideshow"),
        "API route must win over static/json"
    );
    assert!(!body.contains("SHADOWED-FILE-CONTENT"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn api_catalog_stays_dynamic_even_with_all_overrides() {
    let dir = temp_static_dir("catalog");
    for (name, content) in [
        ("openapi.json", r#"{"paths":{}}"#),
        ("favicon.png", "x"),
        ("index.html", "<html></html>"),
        ("robots.txt", "Disallow: *"),
        ("sitemap.xml", "<urlset/>"),
    ] {
        write_static(&dir, name, content);
    }
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;
    let req = test::TestRequest::get()
        .uri("/.well-known/api-catalog")
        .insert_header(("host", "catalog.test"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.starts_with("application/linkset+json"));
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("linkset JSON");
    assert_eq!(
        v["linkset"][0]["anchor"].as_str(),
        Some("http://catalog.test/"),
        "api-catalog is always generated from the request, never overridable"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[actix_web::test]
async fn dynamic_robots_and_sitemap_served_when_no_override() {
    let dir = temp_static_dir("dynamic-seo");
    let app = test::init_service(create_app(cfg_with_static(&dir))).await;

    let req = test::TestRequest::get().uri("/robots.txt").to_request();
    let resp = test::call_service(&app, req).await;
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("Disallow: /deny"));
    assert!(body.contains("Sitemap: "));

    let req = test::TestRequest::get().uri("/sitemap.xml").to_request();
    let resp = test::call_service(&app, req).await;
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("<urlset"));
    assert!(body.contains("<loc>"));
    let _ = std::fs::remove_dir_all(&dir);
}
