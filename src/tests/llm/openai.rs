//! OpenAI chat completions mock (/llm/v1/chat/completions)

use super::*;

#[actix_web::test]
async fn llm_chat_completions_canonical_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"model":"gpt-5-nano","messages":[{"role":"user","content":"Hello"}]}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    // Spec-required top-level fields, exactly (optional service_tier /
    // system_fingerprint deliberately omitted).
    assert_eq!(
        llm_sorted_keys(&v),
        vec!["choices", "created", "id", "model", "object", "usage"]
    );
    assert!(v["id"].as_str().unwrap().starts_with("chatcmpl-"));
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["model"], "gpt-5-nano");
    assert!(v["created"].as_i64().unwrap() > 0);

    let choice = &v["choices"][0];
    assert_eq!(
        llm_sorted_keys(choice),
        vec!["finish_reason", "index", "logprobs", "message"]
    );
    assert_eq!(choice["finish_reason"], "stop");
    assert!(choice["logprobs"].is_null());
    let message = &choice["message"];
    assert_eq!(llm_sorted_keys(message), vec!["content", "refusal", "role"]);
    assert_eq!(message["role"], "assistant");
    assert!(message["refusal"].is_null());
    let content = message["content"].as_str().unwrap();
    assert!(!content.is_empty());

    let usage = &v["usage"];
    assert_eq!(
        llm_sorted_keys(usage),
        vec![
            "completion_tokens",
            "completion_tokens_details",
            "prompt_tokens",
            "prompt_tokens_details",
            "total_tokens"
        ]
    );
    // "user: Hello\n" is 12 chars -> ceil(12/4) = 3 prompt tokens, matching
    // the established heuristic.
    assert_eq!(usage["prompt_tokens"], 3);
    assert_eq!(
        usage["completion_tokens"],
        crate::handlers::llm::estimate_tokens(content)
    );
    assert_eq!(
        usage["total_tokens"],
        usage["prompt_tokens"].as_u64().unwrap() + usage["completion_tokens"].as_u64().unwrap()
    );
    assert!(usage["prompt_tokens_details"].is_null());
}

#[actix_web::test]
async fn llm_chat_completions_custom_content_via_httpcan_block() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"content":"Custom reply!"}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "Custom reply!");
    assert_eq!(
        v["usage"]["completion_tokens"],
        crate::handlers::llm::estimate_tokens("Custom reply!")
    );
}

#[actix_web::test]
async fn llm_chat_completions_n_choices() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"messages":[{"role":"user","content":"hi"}],"n":2}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let choices = v["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0]["index"], 0);
    assert_eq!(choices[1]["index"], 1);
}

#[actix_web::test]
async fn llm_chat_completions_silent_alias_path() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"messages":[{"role":"user","content":"Hello"}]}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["object"], "chat.completion");
    assert!(v["id"].as_str().unwrap().starts_with("chatcmpl-"));
}

#[actix_web::test]
async fn llm_chat_completions_invalid_json_openai_error_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload("not json")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(llm_sorted_keys(&v), vec!["error"]);
    assert_eq!(v["error"]["type"], "invalid_request_error");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Invalid JSON"));
}

#[actix_web::test]
async fn llm_chat_completions_missing_messages_is_400() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"model":"gpt-4o"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
    assert!(v["error"]["message"].as_str().unwrap().contains("messages"));
}

#[actix_web::test]
async fn llm_chat_completions_get_is_405_with_allow_post() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/llm/v1/chat/completions")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        resp.headers().get("Allow").unwrap().to_str().unwrap(),
        "POST"
    );
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
}

#[actix_web::test]
async fn llm_chat_completions_stream_protocol() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"Hello"}],"stream":true,"httpcan":{"content":"word one two three"}}"#,
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
    // OpenAI-family SSE carries no `event:` lines.
    assert!(!text.contains("event:"));

    let frames = llm_sse_frames(&text);
    assert_eq!(frames.last().unwrap(), &"data: [DONE]");
    let chunks: Vec<serde_json::Value> = frames[..frames.len() - 1]
        .iter()
        .map(|f| llm_frame_data(f))
        .collect();

    let id = chunks[0]["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("chatcmpl-"));
    for chunk in &chunks {
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["id"], id.as_str());
        assert!(
            chunk.get("usage").is_none(),
            "usage only appears with include_usage"
        );
    }

    // First chunk: role delta, null finish_reason.
    assert_eq!(chunks[0]["choices"][0]["delta"]["role"], "assistant");
    assert!(chunks[0]["choices"][0]["finish_reason"].is_null());

    // Word chunks reconstruct the content exactly (leading-space scheme).
    let mut reconstructed = String::new();
    for chunk in &chunks[1..chunks.len() - 1] {
        reconstructed.push_str(chunk["choices"][0]["delta"]["content"].as_str().unwrap());
    }
    assert_eq!(reconstructed, "word one two three");

    // Final chunk: empty delta, finish_reason stop.
    let last = chunks.last().unwrap();
    assert_eq!(last["choices"][0]["delta"].as_object().unwrap().len(), 0);
    assert_eq!(last["choices"][0]["finish_reason"], "stop");
}

#[actix_web::test]
async fn llm_chat_completions_stream_include_usage() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"Hello"}],"stream":true,"stream_options":{"include_usage":true}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    let frames = llm_sse_frames(&text);
    assert_eq!(frames.last().unwrap(), &"data: [DONE]");

    // Frame order: ... stop chunk, usage chunk (empty choices), [DONE].
    let usage_frame = llm_frame_data(frames[frames.len() - 2]);
    assert_eq!(usage_frame["choices"].as_array().unwrap().len(), 0);
    assert!(usage_frame["usage"].is_object());
    assert_eq!(usage_frame["usage"]["prompt_tokens"], 3);
    assert_eq!(
        usage_frame["usage"]["total_tokens"],
        usage_frame["usage"]["prompt_tokens"].as_u64().unwrap()
            + usage_frame["usage"]["completion_tokens"].as_u64().unwrap()
    );
    let stop_frame = llm_frame_data(frames[frames.len() - 3]);
    assert_eq!(stop_frame["choices"][0]["finish_reason"], "stop");
}
