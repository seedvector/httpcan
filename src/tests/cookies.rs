//! Cookies: /cookies /cookies/set /cookies/delete

use super::*;

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
