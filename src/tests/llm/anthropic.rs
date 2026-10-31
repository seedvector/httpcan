//! Anthropic messages mock (/llm/v1/messages)

use super::*;

#[actix_web::test]
async fn llm_messages_canonical_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"model":"claude-sonnet-4-5","max_tokens":1024,"messages":[{"role":"user","content":"Hello"}]}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        llm_sorted_keys(&v),
        vec![
            "content",
            "id",
            "model",
            "role",
            "stop_reason",
            "stop_sequence",
            "type",
            "usage"
        ]
    );
    assert!(v["id"].as_str().unwrap().starts_with("msg_"));
    assert_eq!(v["type"], "message");
    assert_eq!(v["role"], "assistant");
    assert_eq!(v["model"], "claude-sonnet-4-5");
    assert_eq!(v["stop_reason"], "end_turn");
    assert!(v["stop_sequence"].is_null());

    let block = &v["content"][0];
    assert_eq!(llm_sorted_keys(block), vec!["citations", "text", "type"]);
    assert_eq!(block["type"], "text");
    let text = block["text"].as_str().unwrap();
    assert!(!text.is_empty());

    let usage = &v["usage"];
    assert_eq!(
        llm_sorted_keys(usage),
        vec!["input_tokens", "output_tokens"]
    );
    assert_eq!(usage["input_tokens"], 3);
    assert_eq!(
        usage["output_tokens"],
        crate::handlers::llm::estimate_tokens(text)
    );
}

#[actix_web::test]
async fn llm_messages_custom_content_and_system_tokens() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"max_tokens":100,"system":"You are terse.","messages":[{"role":"user","content":[{"type":"text","text":"Hello"}]}],"httpcan":{"content":"Hi."}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["content"][0]["text"], "Hi.");
    // system line + user line both counted: "system: You are terse.\n" (23)
    // + "user: Hello\n" (12) = 35 chars -> ceil(35/4) = 9.
    assert_eq!(v["usage"]["input_tokens"], 9);
    // No model in request -> default model echoed.
    assert!(v["model"].as_str().unwrap().starts_with("claude-"));
}

#[actix_web::test]
async fn llm_messages_invalid_json_anthropic_error_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload("not json")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["error"]["type"], "invalid_request_error");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Invalid JSON"));
}

#[actix_web::test]
async fn llm_messages_stream_protocol() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"max_tokens":64,"stream":true,"messages":[{"role":"user","content":"Hello"}],"httpcan":{"content":"alpha beta"}}"#,
        )
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
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(
        !text.contains("[DONE]"),
        "Anthropic SSE has no [DONE] sentinel"
    );

    // Each frame is "event: <type>\ndata: {json}".
    let mut events: Vec<(String, serde_json::Value)> = Vec::new();
    for frame in llm_sse_frames(&text) {
        let (event, data) = frame.split_once('\n').unwrap();
        let event = event.strip_prefix("event: ").unwrap().to_string();
        let data = serde_json::from_str(data.strip_prefix("data: ").unwrap()).unwrap();
        events.push((event, data));
    }

    let types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();
    assert_eq!(
        &types[..],
        &[
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop"
        ]
    );

    let start = &events[0].1;
    assert!(start["message"]["id"].as_str().unwrap().starts_with("msg_"));
    assert_eq!(start["message"]["usage"]["input_tokens"], 3);
    assert_eq!(start["message"]["usage"]["output_tokens"], 0);

    let mut reconstructed = String::new();
    for (event, data) in &events {
        if event == "content_block_delta" {
            assert_eq!(data["delta"]["type"], "text_delta");
            reconstructed.push_str(data["delta"]["text"].as_str().unwrap());
        }
    }
    assert_eq!(reconstructed, "alpha beta");

    let delta = &events[events.len() - 2].1;
    assert_eq!(delta["delta"]["stop_reason"], "end_turn");
    assert_eq!(
        delta["usage"]["output_tokens"],
        crate::handlers::llm::estimate_tokens("alpha beta")
    );
}

#[actix_web::test]
async fn llm_messages_get_is_405_anthropic_error_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/llm/v1/messages")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        resp.headers().get("Allow").unwrap().to_str().unwrap(),
        "POST"
    );
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["type"], "error");
}
