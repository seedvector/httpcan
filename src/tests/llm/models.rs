//! Model catalog (/llm/v1/models) and the /llm index

use super::*;

#[actix_web::test]
async fn llm_models_openai_shape_by_default() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/llm/v1/models").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(llm_sorted_keys(&v), vec!["data", "object"]);
    assert_eq!(v["object"], "list");
    let data = v["data"].as_array().unwrap();
    assert!(!data.is_empty());
    for m in data {
        assert_eq!(
            llm_sorted_keys(m),
            vec!["created", "id", "object", "owned_by"]
        );
        assert_eq!(m["object"], "model");
    }
}

#[actix_web::test]
async fn llm_models_anthropic_shape_via_version_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/llm/v1/models")
        .insert_header(("anthropic-version", "2023-06-01"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        llm_sorted_keys(&v),
        vec!["data", "first_id", "has_more", "last_id"]
    );
    assert_eq!(v["has_more"], false);
    let data = v["data"].as_array().unwrap();
    assert!(!data.is_empty());
    for m in data {
        assert_eq!(
            llm_sorted_keys(m),
            vec![
                "capabilities",
                "created_at",
                "display_name",
                "id",
                "max_input_tokens",
                "max_tokens",
                "type"
            ]
        );
        assert_eq!(m["type"], "model");
    }
    assert_eq!(v["first_id"], data[0]["id"]);
    assert_eq!(v["last_id"], data[data.len() - 1]["id"]);
}

#[actix_web::test]
async fn llm_model_detail_echoes_any_id() {
    let app = test::init_service(create_app(cfg())).await;

    let req = test::TestRequest::get()
        .uri("/llm/v1/models/my-fine-tune-42")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["id"], "my-fine-tune-42");
    assert_eq!(v["object"], "model");

    let req = test::TestRequest::get()
        .uri("/llm/v1/models/claude-opus-9-2099")
        .insert_header(("anthropic-version", "2023-06-01"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["id"], "claude-opus-9-2099");
    assert_eq!(v["type"], "model");
    assert_eq!(v["display_name"], "claude-opus-9-2099");
}

#[actix_web::test]
async fn llm_index_endpoint_lists_contract() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/llm").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(v["endpoints"]["chat_completions"]["path"]
        .as_str()
        .unwrap()
        .starts_with("/llm"));
    assert!(v["base_url"]["anthropic_sdk"]
        .as_str()
        .unwrap()
        .ends_with("/llm"));
}

/// Catalog ids for one request (`anthropic` picks the Anthropic shape).
async fn llm_catalog_ids(config: ServerConfig, anthropic: bool) -> Vec<String> {
    let app = test::init_service(create_app(config)).await;
    let mut req = test::TestRequest::get().uri("/llm/v1/models");
    if anthropic {
        req = req.insert_header(("anthropic-version", "2023-06-01"));
    }
    let resp = test::call_service(&app, req.to_request()).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_string())
        .collect()
}

/// The default catalog is mock-owned and frozen: changing it breaks users'
/// tests, so this assertion is exact on purpose.
#[actix_web::test]
async fn llm_models_default_catalog_is_frozen() {
    assert_eq!(
        llm_catalog_ids(cfg(), false).await,
        vec!["httpcan-openai", "httpcan-auto-tools"]
    );
    assert_eq!(
        llm_catalog_ids(cfg(), true).await,
        vec!["httpcan-anthropic", "httpcan-auto-tools"]
    );
}

#[actix_web::test]
async fn llm_models_configured_catalog() {
    let config = || {
        cfg().llm_models(vec![
            "gpt-5.5".into(),
            "claude-opus-5-5".into(),
            "httpcan-auto-tools".into(),
        ])
    };
    // Shared by both families; auto-tools appended once (deduplicated).
    let want = vec!["gpt-5.5", "claude-opus-5-5", "httpcan-auto-tools"];
    assert_eq!(llm_catalog_ids(config(), false).await, want);
    assert_eq!(llm_catalog_ids(config(), true).await, want);
}

/// Requests that omit `model` echo the frozen defaults, and the defaults
/// are the ids the catalog lists.
#[actix_web::test]
async fn llm_default_models_echoed_and_listed() {
    let app = test::init_service(create_app(cfg())).await;
    for (uri, body, want) in [
        (
            "/llm/v1/chat/completions",
            r#"{"messages":[{"role":"user","content":"hi"}]}"#,
            "httpcan-openai",
        ),
        ("/llm/v1/responses", r#"{"input":"hi"}"#, "httpcan-openai"),
        (
            "/llm/v1/completions",
            r#"{"prompt":"hi"}"#,
            "httpcan-openai",
        ),
        (
            "/llm/v1/messages",
            r#"{"max_tokens":8,"messages":[{"role":"user","content":"hi"}]}"#,
            "httpcan-anthropic",
        ),
    ] {
        let req = test::TestRequest::post()
            .uri(uri)
            .insert_header(("content-type", "application/json"))
            .set_payload(body)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
        assert_eq!(v["model"], want, "{uri}");
    }
    assert!(llm_catalog_ids(cfg(), false)
        .await
        .contains(&"httpcan-openai".to_string()));
    assert!(llm_catalog_ids(cfg(), true)
        .await
        .contains(&"httpcan-anthropic".to_string()));
}
