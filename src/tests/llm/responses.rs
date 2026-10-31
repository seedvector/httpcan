//! OpenAI Responses API mock (/llm/v1/responses)

use super::*;

#[actix_web::test]
async fn llm_responses_canonical_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"model":"gpt-5.6-sol","input":"Hello"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        llm_sorted_keys(&v),
        vec![
            "completed_at",
            "created_at",
            "error",
            "id",
            "incomplete_details",
            "instructions",
            "max_output_tokens",
            "metadata",
            "model",
            "object",
            "output",
            "output_text",
            "parallel_tool_calls",
            "status",
            "temperature",
            "tool_choice",
            "tools",
            "top_p",
            "usage"
        ]
    );
    assert!(v["id"].as_str().unwrap().starts_with("resp_"));
    assert_eq!(v["object"], "response");
    assert_eq!(v["status"], "completed");
    assert_eq!(v["model"], "gpt-5.6-sol");
    assert!(v["error"].is_null());
    assert!(v["incomplete_details"].is_null());
    assert_eq!(v["temperature"], 1.0);
    assert_eq!(v["top_p"], 1.0);
    assert_eq!(v["tool_choice"], "auto");
    assert_eq!(v["parallel_tool_calls"], false);
    assert_eq!(v["metadata"].as_object().unwrap().len(), 0);

    let item = &v["output"][0];
    assert_eq!(
        llm_sorted_keys(item),
        vec!["content", "id", "role", "status", "type"]
    );
    assert!(item["id"].as_str().unwrap().starts_with("msg_"));
    assert_eq!(item["type"], "message");
    assert_eq!(item["role"], "assistant");
    assert_eq!(item["status"], "completed");
    let part = &item["content"][0];
    assert_eq!(llm_sorted_keys(part), vec!["annotations", "text", "type"]);
    assert_eq!(part["type"], "output_text");
    assert_eq!(part["annotations"].as_array().unwrap().len(), 0);

    let text = part["text"].as_str().unwrap();
    assert_eq!(v["output_text"], text);

    let usage = &v["usage"];
    assert_eq!(
        llm_sorted_keys(usage),
        vec![
            "input_tokens",
            "input_tokens_details",
            "output_tokens",
            "output_tokens_details",
            "total_tokens"
        ]
    );
    assert_eq!(usage["input_tokens"], 2); // "Hello" -> ceil(5/4)
    assert_eq!(usage["input_tokens_details"]["cached_tokens"], 0);
    assert_eq!(usage["output_tokens_details"]["reasoning_tokens"], 0);
    assert_eq!(
        usage["total_tokens"],
        usage["input_tokens"].as_u64().unwrap() + usage["output_tokens"].as_u64().unwrap()
    );
}

#[actix_web::test]
async fn llm_responses_custom_content_and_instructions_echo() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"input":[{"role":"user","content":[{"type":"input_text","text":"Hello"}]}],"instructions":"Be terse.","httpcan":{"content":"Ok."}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["output_text"], "Ok.");
    assert_eq!(v["instructions"], "Be terse.");
    // "user: Hello" -> 11 chars -> ceil(11/4) = 3
    assert_eq!(v["usage"]["input_tokens"], 3);
}

#[actix_web::test]
async fn llm_responses_invalid_json_openai_error_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload("not json")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
}

#[actix_web::test]
async fn llm_responses_get_is_405_with_allow_post() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/llm/v1/responses")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        resp.headers().get("Allow").unwrap().to_str().unwrap(),
        "POST"
    );
}

#[actix_web::test]
async fn llm_responses_silent_alias_path() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"input":"Hello"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["object"], "response");
    assert_eq!(v["model"], "gpt-5.6-sol");
}

#[actix_web::test]
async fn llm_responses_stream_protocol() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"input":"Hello","stream":true,"httpcan":{"content":"one two three"}}"#)
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
    // Responses streaming uses named events and no [DONE] sentinel.
    assert!(
        !text.contains("[DONE]"),
        "Responses streams end with response.completed, not [DONE]"
    );

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
            "response.created",
            "response.in_progress",
            "response.output_item.added",
            "response.content_part.added",
            "response.output_text.delta",
            "response.output_text.delta",
            "response.output_text.delta",
            "response.output_text.done",
            "response.content_part.done",
            "response.output_item.done",
            "response.completed",
        ]
    );

    // sequence_number strictly increases from 0.
    for (i, (_, data)) in events.iter().enumerate() {
        assert_eq!(data["sequence_number"], i as u64);
    }

    let created = &events[0].1;
    assert_eq!(created["response"]["status"], "in_progress");
    assert_eq!(created["response"]["object"], "response");

    let mut reconstructed = String::new();
    for (event, data) in &events {
        if event == "response.output_text.delta" {
            reconstructed.push_str(data["delta"].as_str().unwrap());
        }
    }
    assert_eq!(reconstructed, "one two three");

    let done = &events[7].1;
    assert_eq!(done["text"], "one two three");

    let completed = &events.last().unwrap().1;
    let final_response = &completed["response"];
    assert_eq!(final_response["status"], "completed");
    assert_eq!(final_response["output_text"], "one two three");
    assert_eq!(final_response["usage"]["input_tokens"], 2);
    assert_eq!(
        final_response["usage"]["total_tokens"],
        final_response["usage"]["input_tokens"].as_u64().unwrap()
            + final_response["usage"]["output_tokens"].as_u64().unwrap()
    );
}
