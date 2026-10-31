//! OpenAPI spec (/openapi.json)

use super::*;

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

/// The spec's category tags for the `/llm` and `/oauth2` surfaces match the
/// homepage category titles exactly ("AI API Mock", "OAuth 2.0 Mock"), every
/// documented mock path references its tag, and the Bearer-enforced userinfo
/// operations declare the `bearerAuth` security scheme. Guards the homepage ↔
/// spec ↔ README naming alignment. Silent aliases and `/.well-known/*`
/// discovery documents exist as routes but are not interfaces, so they stay
/// out of the spec and the homepage endpoint list.
#[actix_web::test]
async fn openapi_tags_match_homepage_categories_for_mock_surfaces() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");

    let tag_names: Vec<&str> = v["tags"]
        .as_array()
        .expect("tags array")
        .iter()
        .map(|t| t["name"].as_str().expect("tag name"))
        .collect();
    assert!(
        tag_names.contains(&"AI API Mock"),
        "tags must contain 'AI API Mock' (homepage category title), got {tag_names:?}"
    );
    assert!(
        tag_names.contains(&"OAuth 2.0 Mock"),
        "tags must contain 'OAuth 2.0 Mock' (homepage category title), got {tag_names:?}"
    );

    let expected: &[(&str, &str)] = &[
        ("/llm", "AI API Mock"),
        ("/llm/v1/chat/completions", "AI API Mock"),
        ("/llm/v1/messages", "AI API Mock"),
        ("/llm/v1/responses", "AI API Mock"),
        ("/llm/v1/completions", "AI API Mock"),
        ("/llm/v1/models", "AI API Mock"),
        ("/llm/v1/models/{model}", "AI API Mock"),
        ("/oauth2", "OAuth 2.0 Mock"),
        ("/oauth2/authorize", "OAuth 2.0 Mock"),
        ("/oauth2/token", "OAuth 2.0 Mock"),
        ("/oauth2/userinfo", "OAuth 2.0 Mock"),
    ];
    for (path, tag) in expected {
        let ops = v["paths"][path]
            .as_object()
            .unwrap_or_else(|| panic!("path {path} missing from spec"));
        for (method, op) in ops {
            assert_eq!(
                op["tags"][0].as_str(),
                Some(*tag),
                "{method} {path} must be tagged {tag}"
            );
        }
    }
    // Silent aliases and RFC well-known discovery documents exist as routes
    // but are not interfaces: neither may appear in the spec.
    for absent in [
        "/llm/chat/completions",
        "/llm/responses",
        "/llm/completions",
        "/.well-known/api-catalog",
        "/.well-known/oauth-authorization-server",
    ] {
        assert!(
            v["paths"].get(absent).is_none(),
            "{absent} is not an interface and must not appear in the spec"
        );
    }

    for method in ["get", "post"] {
        assert_eq!(
            v["paths"]["/oauth2/userinfo"][method]["security"],
            serde_json::json!([{"bearerAuth": []}]),
            "userinfo {method} enforces Bearer auth and must declare it"
        );
    }

    // The homepage renders the same category titles as the spec tags.
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(
        body.contains("AI API Mock") && body.contains("OAuth 2.0 Mock"),
        "homepage must render the canonical category titles"
    );

    // Same rule for the homepage endpoint list (prose mentions are fine).
    assert!(
        !body.contains("/.well-known/api-catalog")
            && !body.contains("/.well-known/oauth-authorization-server"),
        "homepage endpoint list must not present well-known discovery documents as endpoints"
    );
}

/// GET /.well-known/api-catalog returns 200 with the RFC 9727 Linkset media
/// type (application/linkset+json, profile=rfc9727). The single entry's anchor
/// is the API root, with service-desc (OpenAPI spec), service-doc (homepage),
/// and status (health probe) relations whose hrefs target the request origin.
#[actix_web::test]
async fn api_catalog_returns_rfc9727_linkset() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/.well-known/api-catalog")
        .insert_header(("Host", "example.com"))
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
        ct.starts_with("application/linkset+json"),
        "expected application/linkset+json content-type, got {ct}"
    );
    assert!(
        ct.contains("rfc9727"),
        "content-type should carry the RFC 9727 profile parameter, got {ct}"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    let linkset = v
        .get("linkset")
        .and_then(|l| l.as_array())
        .expect("linkset array");
    assert!(!linkset.is_empty(), "linkset must list at least one API");
    let entry = &linkset[0];
    // anchor is an absolute URL at the API root.
    assert_eq!(
        entry.get("anchor").and_then(|a| a.as_str()),
        Some("http://example.com/")
    );
    // service-desc -> OpenAPI spec.
    let desc = entry
        .get("service-desc")
        .and_then(|s| s.as_array())
        .expect("service-desc array");
    assert_eq!(
        desc[0].get("href").and_then(|h| h.as_str()),
        Some("http://example.com/openapi.json")
    );
    assert_eq!(
        desc[0].get("type").and_then(|t| t.as_str()),
        Some("application/json")
    );
    // service-doc -> homepage.
    let doc = entry
        .get("service-doc")
        .and_then(|s| s.as_array())
        .expect("service-doc array");
    assert_eq!(
        doc[0].get("href").and_then(|h| h.as_str()),
        Some("http://example.com/")
    );
    assert_eq!(
        doc[0].get("type").and_then(|t| t.as_str()),
        Some("text/html")
    );
    // status -> health probe.
    let status = entry
        .get("status")
        .and_then(|s| s.as_array())
        .expect("status array");
    assert_eq!(
        status[0].get("href").and_then(|h| h.as_str()),
        Some("http://example.com/healthz")
    );
    assert_eq!(
        status[0].get("type").and_then(|t| t.as_str()),
        Some("application/json")
    );
}
