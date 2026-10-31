//! Observability: /healthz /tags

use super::*;

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
