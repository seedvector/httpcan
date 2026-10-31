//! Redirects: /redirect /relative-redirect /absolute-redirect /redirect-to

use super::*;

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
