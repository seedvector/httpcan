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
