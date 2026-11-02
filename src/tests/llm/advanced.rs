//! Advanced mock-script features (httpcan override block):
//! tool_calls / error injection / usage pinning / delay / sequence.
//! Design: internal/llm-mock-advanced-design.md.

use super::*;

// ---------------------------------------------------------------- tool_calls

#[actix_web::test]
async fn llm_chat_completions_tool_calls_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"weather?"}],"httpcan":{"content":"Let me check.","tool_calls":[{"id":"call_fixed","name":"get_weather","arguments":{"city":"SF"}}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let message = &v["choices"][0]["message"];
    // content and tool_calls coexist, as the real API allows.
    assert_eq!(message["content"], "Let me check.");
    let calls = message["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["id"], "call_fixed");
    assert_eq!(calls[0]["type"], "function");
    assert_eq!(calls[0]["function"]["name"], "get_weather");
    // arguments is the JSON-encoded string, not an object.
    assert_eq!(calls[0]["function"]["arguments"], r#"{"city":"SF"}"#);
    assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");
    // usage covers text plus serialized tool calls.
    assert!(v["usage"]["completion_tokens"].as_u64().unwrap() > 0);
}

#[actix_web::test]
async fn llm_chat_completions_generated_call_id_prefix() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"tool_calls":[{"name":"f"}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let id = v["choices"][0]["message"]["tool_calls"][0]["id"]
        .as_str()
        .unwrap();
    assert!(id.starts_with("call_"), "got {id}");
    // Arguments default to the empty object, JSON-encoded.
    assert_eq!(
        v["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        "{}"
    );
}

#[actix_web::test]
async fn llm_chat_completions_tool_choice_required_synthesizes_call() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"tools":[{"type":"function","function":{"name":"first_tool"}},{"type":"function","function":{"name":"second_tool"}}],"tool_choice":"required"}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let call = &v["choices"][0]["message"]["tool_calls"][0];
    // Falls back to the first declared tool with empty arguments.
    assert_eq!(call["function"]["name"], "first_tool");
    assert_eq!(call["function"]["arguments"], "{}");
    assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");
}

#[actix_web::test]
async fn llm_chat_completions_tool_choice_named_overrides_first_tool() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"tools":[{"type":"function","function":{"name":"first_tool"}}],"tool_choice":{"type":"function","function":{"name":"picked_tool"}}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        v["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "picked_tool"
    );
}

#[actix_web::test]
async fn llm_chat_completions_stream_tool_call_frames() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"stream":true,"httpcan":{"content":"","tool_calls":[{"id":"call_x","name":"get_weather","arguments":{"city":"SF"}}],"stream_chunk_delay_ms":0}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    let frames = llm_sse_frames(&text);
    assert_eq!(frames.last().unwrap(), &"data: [DONE]");
    let chunks: Vec<serde_json::Value> = frames[..frames.len() - 1]
        .iter()
        .map(|f| llm_frame_data(f))
        .collect();

    // role delta, tool-call open delta, arguments delta, finish chunk.
    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks[0]["choices"][0]["delta"]["role"], "assistant");

    let open = &chunks[1]["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(open["index"], 0);
    assert_eq!(open["id"], "call_x");
    assert_eq!(open["type"], "function");
    assert_eq!(open["function"]["name"], "get_weather");
    assert_eq!(open["function"]["arguments"], "");

    let args = &chunks[2]["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(args["index"], 0);
    assert_eq!(args["function"]["arguments"], r#"{"city":"SF"}"#);

    assert_eq!(chunks[3]["choices"][0]["finish_reason"], "tool_calls");
}

#[actix_web::test]
async fn llm_anthropic_tool_use_blocks() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"model":"claude-sonnet-5","max_tokens":100,"messages":[{"role":"user","content":"weather?"}],"httpcan":{"tool_calls":[{"name":"get_weather","arguments":{"city":"SF"}}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let blocks = v["content"].as_array().unwrap();
    // Tool-only answer (no `content` set): no placeholder text block, just
    // one tool_use block per call.
    assert_eq!(blocks.len(), 1);
    let tool = &blocks[0];
    assert_eq!(tool["type"], "tool_use");
    assert_eq!(tool["name"], "get_weather");
    // Anthropic transports arguments as the `input` object.
    assert_eq!(tool["input"]["city"], "SF");
    assert!(tool["id"].as_str().unwrap().starts_with("toolu_"));
    assert_eq!(v["stop_reason"], "tool_use");
}

#[actix_web::test]
async fn llm_anthropic_stream_tool_use_events() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"stream":true,"httpcan":{"content":"","tool_calls":[{"name":"f","arguments":{"a":1}}],"stream_chunk_delay_ms":0}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    // Each Anthropic frame is "event: <type>\ndata: {json}".
    let events: Vec<String> = llm_sse_frames(&text)
        .iter()
        .map(|f| {
            f.split_once('\n')
                .unwrap()
                .0
                .strip_prefix("event: ")
                .unwrap()
                .to_string()
        })
        .collect();
    let events: Vec<&str> = events.iter().map(String::as_str).collect();
    assert_eq!(
        events,
        vec![
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop",
        ]
    );

    let frames = llm_sse_frames(&text);
    let data = |frame: &str| -> serde_json::Value {
        serde_json::from_str(
            frame
                .split_once('\n')
                .unwrap()
                .1
                .strip_prefix("data: ")
                .unwrap(),
        )
        .unwrap()
    };
    // Tool-only answer: no empty text block, so the tool block is index 0.
    let start = data(frames[1]);
    assert_eq!(start["index"], 0);
    assert_eq!(start["content_block"]["type"], "tool_use");
    assert_eq!(start["content_block"]["name"], "f");
    let delta = data(frames[2]);
    assert_eq!(delta["delta"]["type"], "input_json_delta");
    assert_eq!(delta["delta"]["partial_json"], r#"{"a":1}"#);
    let message_delta = data(frames[4]);
    assert_eq!(message_delta["delta"]["stop_reason"], "tool_use");
}

#[actix_web::test]
async fn llm_anthropic_tool_choice_any_synthesizes_call() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"tools":[{"name":"lookup","input_schema":{"type":"object"}}],"tool_choice":{"type":"any"}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let blocks = v["content"].as_array().unwrap();
    assert_eq!(blocks[blocks.len() - 1]["type"], "tool_use");
    assert_eq!(blocks[blocks.len() - 1]["name"], "lookup");
    assert_eq!(v["stop_reason"], "tool_use");
}

#[actix_web::test]
async fn llm_responses_function_call_output_item() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"input":"hi","httpcan":{"content":"","tool_calls":[{"name":"get_weather","arguments":{"city":"SF"}}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    // Empty content suppresses the message item: only the function_call.
    let output = v["output"].as_array().unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["type"], "function_call");
    assert_eq!(output[0]["name"], "get_weather");
    assert_eq!(output[0]["arguments"], r#"{"city":"SF"}"#);
    assert!(output[0]["id"].as_str().unwrap().starts_with("fc_"));
    assert!(output[0]["call_id"].as_str().unwrap().starts_with("call_"));
    assert_eq!(v["status"], "completed");
}

#[actix_web::test]
async fn llm_completions_tool_calls_rejected() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"prompt":"hi","httpcan":{"tool_calls":[{"name":"f"}]}}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    // The legacy API has no tools: explicit 400, never a silent degradation.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("legacy completions"));
}

// ------------------------------------------------------------ error injection

#[actix_web::test]
async fn llm_chat_completions_error_rate_limit_openai_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"error":"rate_limit"}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "rate_limit_error");
    assert_eq!(v["error"]["code"], "rate_limit_exceeded");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Rate limit"));
}

#[actix_web::test]
async fn llm_anthropic_error_overloaded_529_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"error":"overloaded"}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    // Anthropic's vendor-specific overload status.
    assert_eq!(resp.status().as_u16(), 529);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["error"]["type"], "overloaded_error");
}

#[actix_web::test]
async fn llm_chat_completions_error_content_filter_in_band() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"error":"content_filter","refusal":"can't do that"}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    // In-band: 200 with the refusal field and content_filter finish.
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(v["choices"][0]["message"]["content"].is_null());
    assert_eq!(v["choices"][0]["message"]["refusal"], "can't do that");
    assert_eq!(v["choices"][0]["finish_reason"], "content_filter");
}

#[actix_web::test]
async fn llm_anthropic_error_content_filter_in_band() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"error":"content_filter"}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["stop_reason"], "refusal");
    let blocks = v["content"].as_array().unwrap();
    assert_eq!(blocks[0]["type"], "text");
    assert!(!blocks[0]["text"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn llm_completions_error_invalid_api_key() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"prompt":"hi","httpcan":{"error":"invalid_api_key"}}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["code"], "invalid_api_key");
}

#[actix_web::test]
async fn llm_responses_error_context_length() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"input":"hi","httpcan":{"error":"context_length"}}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
    assert_eq!(v["error"]["code"], "context_length_exceeded");
}

// -------------------------------------------------------------- usage pinning

#[actix_web::test]
async fn llm_chat_completions_usage_pinned_exact() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"usage":{"prompt_tokens":10,"completion_tokens":5}}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    // Verbatim, not the heuristic estimate ("user: hi\n" would be 3).
    assert_eq!(v["usage"]["prompt_tokens"], 10);
    assert_eq!(v["usage"]["completion_tokens"], 5);
    assert_eq!(v["usage"]["total_tokens"], 15);
}

#[actix_web::test]
async fn llm_anthropic_usage_pinned_maps_field_names() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"usage":{"prompt_tokens":7,"completion_tokens":4}}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["usage"]["input_tokens"], 7);
    assert_eq!(v["usage"]["output_tokens"], 4);
}

// -------------------------------------------------------- delay and stream pacing

#[actix_web::test]
async fn llm_chat_completions_delay_ms_delays_response() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"delay_ms":150}}"#)
        .to_request();
    let start = std::time::Instant::now();
    let resp = test::call_service(&app, req).await;
    let elapsed = start.elapsed();

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        elapsed >= std::time::Duration::from_millis(140),
        "elapsed {elapsed:?} < 140ms"
    );
}

#[actix_web::test]
async fn llm_chat_completions_stream_zero_chunk_delay_is_fast() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"hi"}],"stream":true,"httpcan":{"content":"a b c d e f g h i j","stream_chunk_delay_ms":0}}"#,
        )
        .to_request();
    let start = std::time::Instant::now();
    let resp = test::call_service(&app, req).await;
    let elapsed = start.elapsed();

    assert_eq!(resp.status(), StatusCode::OK);
    // 10 deltas at the default 50ms would take >= 500ms; zero-delay must be
    // far below that.
    assert!(
        elapsed < std::time::Duration::from_millis(400),
        "elapsed {elapsed:?}"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(text.contains("data: [DONE]"));
}

#[actix_web::test]
async fn llm_chat_completions_stream_long_content_degenerates_to_600_deltas() {
    let app = test::init_service(create_app(cfg())).await;
    let long = "w ".repeat(1200);
    let body = format!(
        r#"{{"messages":[{{"role":"user","content":"hi"}}],"stream":true,"httpcan":{{"content":"{long}","stream_chunk_delay_ms":0}}}}"#
    );
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    let frames = llm_sse_frames(&text);
    let chunks: Vec<serde_json::Value> = frames[..frames.len() - 1]
        .iter()
        .map(|f| llm_frame_data(f))
        .collect();
    // role delta + <=600 content deltas + finish chunk.
    assert!(chunks.len() <= 602, "got {} chunks", chunks.len());
    // Reconstruction stays exact despite the multi-word grouping.
    let reconstructed: String = chunks[1..chunks.len() - 1]
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["content"].as_str())
        .collect();
    let expected: String = "w ".repeat(1200);
    assert_eq!(reconstructed.trim(), expected.trim());
}

// ------------------------------------------------------------------- sequence

#[actix_web::test]
async fn llm_chat_completions_sequence_advances_on_tool_message() {
    let app = test::init_service(create_app(cfg())).await;

    let step0 = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"weather?"}],"httpcan":{"sequence":[{"tool_calls":[{"name":"get_weather","arguments":{"city":"SF"}}]},{"content":"72F and sunny"}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, step0).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        v["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "get_weather"
    );
    assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");

    // Second turn: the conversation now carries one tool result -> step 1.
    let step1 = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"weather?"},{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{}"}}]},{"role":"tool","tool_call_id":"call_1","content":"72F"}],"httpcan":{"sequence":[{"tool_calls":[{"name":"get_weather","arguments":{"city":"SF"}}]},{"content":"72F and sunny"}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, step1).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "72F and sunny");
    assert_eq!(v["choices"][0]["finish_reason"], "stop");

    // Past the last step, the final step repeats.
    let step2 = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"x"},{"role":"tool","content":"1"},{"role":"tool","content":"2"}],"httpcan":{"sequence":[{"content":"first"},{"content":"last"}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, step2).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "last");
}

#[actix_web::test]
async fn llm_anthropic_sequence_advances_on_tool_result_block() {
    let app = test::init_service(create_app(cfg())).await;

    let step1 = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"72F"}]}],"httpcan":{"sequence":[{"content":"calling"},{"content":"72F and sunny"}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, step1).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["content"][0]["text"], "72F and sunny");
    assert_eq!(v["stop_reason"], "end_turn");
}

#[actix_web::test]
async fn llm_responses_sequence_advances_on_function_call_output() {
    let app = test::init_service(create_app(cfg())).await;

    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"input":[{"role":"user","content":"hi"},{"type":"function_call_output","call_id":"call_1","output":"72F"}],"httpcan":{"sequence":[{"content":"calling"},{"content":"72F and sunny"}]}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["output_text"], "72F and sunny");
}

// --------------------------------------------------------------- proof header

#[actix_web::test]
async fn llm_responses_carry_mock_proof_header() {
    let app = test::init_service(create_app(cfg())).await;

    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"messages":[{"role":"user","content":"hi"}]}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.headers().get("x-httpcan-mock").unwrap(),
        "true",
        "every /llm response proves it came from the mock"
    );

    // Non-LLM endpoints never claim it.
    let req = test::TestRequest::get().uri("/get").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.headers().get("x-httpcan-mock").is_none());
}

// ------------------------------------------------------ #mock prompt directives

#[actix_web::test]
async fn llm_prompt_mock_error_rate_limit() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"please answer #mock error=rate_limit"}]}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "rate_limit_error");
}

#[actix_web::test]
async fn llm_prompt_mock_error_overloaded_anthropic_529() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"messages":[{"role":"user","content":"hi\n#mock error=overloaded"}]}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status().as_u16(), 529);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "overloaded_error");
}

#[actix_web::test]
async fn llm_prompt_mock_usage_and_combined_directives() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r##"{"messages":[{"role":"user","content":"#mock usage=10x5 delay=120"}]}"##)
        .to_request();
    let start = std::time::Instant::now();
    let resp = test::call_service(&app, req).await;
    let elapsed = start.elapsed();

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        elapsed >= std::time::Duration::from_millis(110),
        "{elapsed:?}"
    );
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["usage"]["prompt_tokens"], 10);
    assert_eq!(v["usage"]["completion_tokens"], 5);
}

#[actix_web::test]
async fn llm_prompt_mock_tool_directive() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"weather? #mock tool=get_weather"}]}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let call = &v["choices"][0]["message"]["tool_calls"][0];
    assert_eq!(call["function"]["name"], "get_weather");
    assert_eq!(call["function"]["arguments"], "{}");
    assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");
}

#[actix_web::test]
async fn llm_prompt_mock_anthropic_tool_directive() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(r##"{"messages":[{"role":"user","content":"#mock tool=lookup"}]}"##)
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let blocks = v["content"].as_array().unwrap();
    assert_eq!(blocks[blocks.len() - 1]["type"], "tool_use");
    assert_eq!(blocks[blocks.len() - 1]["name"], "lookup");
    assert_eq!(v["stop_reason"], "tool_use");
}

#[actix_web::test]
async fn llm_prompt_mock_directive_loses_to_explicit_block() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r##"{"messages":[{"role":"user","content":"#mock error=rate_limit"}],"httpcan":{"error":"invalid_api_key"}}"##,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    // What the caller sent on purpose beats what the user typed.
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["code"], "invalid_api_key");
}

#[actix_web::test]
async fn llm_prompt_mock_only_last_user_message_triggers() {
    let app = test::init_service(create_app(cfg())).await;
    // The directive sits in an older user message; the last one is clean.
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r##"{"messages":[{"role":"user","content":"#mock error=rate_limit"},{"role":"assistant","content":"ok"},{"role":"user","content":"hello"}]}"##,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn llm_prompt_mock_requires_standalone_token() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"messages":[{"role":"user","content":"what does #mockery mean? also rate_limit"}]}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    // `#mockery` is not the sigil; normal conversation never triggers.
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn llm_completions_prompt_mock_directive() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r##"{"prompt":"#mock error=invalid_api_key"}"##)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["code"], "invalid_api_key");
}

#[actix_web::test]
async fn llm_responses_prompt_mock_directive() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/responses")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"input":[{"role":"user","content":"hi #mock usage=7x4"}]} "#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["usage"]["input_tokens"], 7);
    assert_eq!(v["usage"]["output_tokens"], 4);
}

// --------------------------------------------------------- auto-tools model

#[actix_web::test]
async fn llm_auto_tools_model_calls_first_declared_tool() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"model":"auto-tools","messages":[{"role":"user","content":"hi"}],"tools":[{"type":"function","function":{"name":"first_tool"}},{"type":"function","function":{"name":"second_tool"}}]}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let call = &v["choices"][0]["message"]["tool_calls"][0];
    assert_eq!(call["function"]["name"], "first_tool");
    assert_eq!(v["choices"][0]["finish_reason"], "tool_calls");
}

#[actix_web::test]
async fn llm_auto_tools_model_echoes_tool_result_on_second_turn() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/chat/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"model":"auto-tools","messages":[{"role":"user","content":"hi"},{"role":"assistant","content":null,"tool_calls":[{"id":"c1","type":"function","function":{"name":"t","arguments":"{}"}}]},{"role":"tool","tool_call_id":"c1","content":"72F"}],"tools":[{"type":"function","function":{"name":"t"}}]}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "Tool result: 72F");
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
}

#[actix_web::test]
async fn llm_auto_tools_anthropic_loop() {
    let app = test::init_service(create_app(cfg())).await;
    let tools = r#"[{"name":"lookup","input_schema":{"type":"object"}}]"#;

    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(format!(
            r#"{{"model":"auto-tools","messages":[{{"role":"user","content":"x"}}],"tools":{tools}}}"#
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let blocks = v["content"].as_array().unwrap();
    assert_eq!(blocks[blocks.len() - 1]["type"], "tool_use");
    assert_eq!(blocks[blocks.len() - 1]["name"], "lookup");

    let req = test::TestRequest::post()
        .uri("/llm/v1/messages")
        .insert_header(("content-type", "application/json"))
        .set_payload(format!(
            r#"{{"model":"auto-tools","messages":[{{"role":"user","content":"x"}},{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t1","content":"done"}}]}}],"tools":{tools}}}"#
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["content"][0]["text"], "Tool result: done");
    assert_eq!(v["stop_reason"], "end_turn");
}

#[actix_web::test]
async fn llm_models_list_includes_auto_tools() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/llm/v1/models").to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let ids: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();
    assert!(ids.contains(&"auto-tools"), "{ids:?}");
}

// ------------------------------------------------------- review regressions

/// POST a JSON body and return (status, raw body).
async fn llm_post(uri: &str, body: &str) -> (StatusCode, String) {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri(uri)
        .insert_header(("content-type", "application/json"))
        .set_payload(body.to_string())
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    (status, body)
}

/// Data payloads of a named-event (Anthropic / Responses) SSE body.
fn llm_event_payloads(text: &str) -> Vec<serde_json::Value> {
    llm_sse_frames(text)
        .iter()
        .map(|f| serde_json::from_str(f.split_once("\ndata: ").unwrap().1).unwrap())
        .collect()
}

#[actix_web::test]
async fn llm_responses_stream_function_call_ids_stable() {
    let (_, text) = llm_post(
        "/llm/v1/responses",
        r#"{"input":"hi","stream":true,"httpcan":{"tool_calls":[{"name":"f"}],"stream_chunk_delay_ms":0}}"#,
    )
    .await;
    let events = llm_event_payloads(&text);
    let added = events
        .iter()
        .find(|e| e["type"] == "response.output_item.added" && e["item"]["type"] == "function_call")
        .unwrap();
    let done = events
        .iter()
        .find(|e| e["type"] == "response.output_item.done" && e["item"]["type"] == "function_call")
        .unwrap();
    let delta = events
        .iter()
        .find(|e| e["type"] == "response.function_call_arguments.delta")
        .unwrap();
    let completed = events.last().unwrap();
    let final_item = &completed["response"]["output"][0];

    let call_id = added["item"]["call_id"].as_str().unwrap();
    let item_id = added["item"]["id"].as_str().unwrap();
    assert!(call_id.starts_with("call_"));
    assert_eq!(done["item"]["call_id"], call_id);
    assert_eq!(final_item["call_id"], call_id);
    assert_eq!(done["item"]["id"], item_id);
    assert_eq!(delta["item_id"], item_id);
    assert_eq!(final_item["id"], item_id);
}

#[actix_web::test]
async fn llm_chat_completions_stream_honors_delay_ms() {
    let start = std::time::Instant::now();
    let (status, _) = llm_post(
        "/llm/v1/chat/completions",
        r#"{"messages":[{"role":"user","content":"hi"}],"stream":true,"httpcan":{"delay_ms":150,"stream_chunk_delay_ms":0}}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(start.elapsed() >= std::time::Duration::from_millis(140));
}

#[actix_web::test]
async fn llm_prompt_mock_tool_directive_fires_once_in_tool_loop() {
    // Turn 2 of a loop started by `#mock tool=…`: the tool result follows the
    // directive message, so the loop ends with a text answer.
    let (_, body) = llm_post(
        "/llm/v1/chat/completions",
        r##"{"messages":[{"role":"user","content":"#mock tool=get_weather"},{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{}"}}]},{"role":"tool","tool_call_id":"call_1","content":"72F"}]}"##,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["choices"][0]["message"].get("tool_calls").is_none());
    assert_eq!(v["choices"][0]["finish_reason"], "stop");

    let (_, body) = llm_post(
        "/llm/v1/responses",
        r##"{"input":[{"role":"user","content":"#mock tool=f"},{"type":"function_call","call_id":"call_1","name":"f","arguments":"{}"},{"type":"function_call_output","call_id":"call_1","output":"ok"}]}"##,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["output"][0]["type"], "message");
}

#[actix_web::test]
async fn llm_tool_only_answers_carry_no_placeholder_text() {
    let (_, body) = llm_post(
        "/llm/v1/chat/completions",
        r#"{"messages":[{"role":"user","content":"hi"}],"httpcan":{"tool_calls":[{"name":"f"}]}}"#,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["choices"][0]["message"]["content"].is_null());

    let (_, body) = llm_post(
        "/llm/v1/responses",
        r#"{"input":"hi","httpcan":{"tool_calls":[{"name":"f"}]}}"#,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["output"].as_array().unwrap().len(), 1);
    assert_eq!(v["output"][0]["type"], "function_call");
}

#[actix_web::test]
async fn llm_responses_prompt_mock_on_plain_string_input() {
    let (status, _) = llm_post(
        "/llm/v1/responses",
        r##"{"input":"#mock error=rate_limit"}"##,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[actix_web::test]
async fn llm_sequence_parallel_tool_results_advance_one_step() {
    let seq = r#"{"sequence":[{"tool_calls":[{"name":"a"},{"name":"b"}]},{"content":"step2"},{"content":"final"}]}"#;

    let (_, body) = llm_post(
        "/llm/v1/chat/completions",
        &format!(
            r#"{{"messages":[{{"role":"user","content":"go"}},{{"role":"assistant","content":null,"tool_calls":[]}},{{"role":"tool","tool_call_id":"1","content":"x"}},{{"role":"tool","tool_call_id":"2","content":"y"}}],"httpcan":{seq}}}"#
        ),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "step2");

    let (_, body) = llm_post(
        "/llm/v1/messages",
        &format!(
            r#"{{"messages":[{{"role":"user","content":"go"}},{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"1","content":"x"}},{{"type":"tool_result","tool_use_id":"2","content":"y"}}]}}],"httpcan":{seq}}}"#
        ),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["content"][0]["text"], "step2");

    let (_, body) = llm_post(
        "/llm/v1/responses",
        &format!(
            r#"{{"input":[{{"role":"user","content":"go"}},{{"type":"function_call","call_id":"1","name":"a","arguments":"{{}}"}},{{"type":"function_call","call_id":"2","name":"b","arguments":"{{}}"}},{{"type":"function_call_output","call_id":"1","output":"x"}},{{"type":"function_call_output","call_id":"2","output":"y"}}],"httpcan":{seq}}}"#
        ),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["output_text"], "step2");
}

#[actix_web::test]
async fn llm_responses_stream_and_json_output_match_for_tool_only() {
    let script =
        r#""httpcan":{"content":"","tool_calls":[{"name":"f"}],"stream_chunk_delay_ms":0}"#;
    let (_, body) = llm_post(
        "/llm/v1/responses",
        &format!(r#"{{"input":"hi",{script}}}"#),
    )
    .await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let (_, text) = llm_post(
        "/llm/v1/responses",
        &format!(r#"{{"input":"hi","stream":true,{script}}}"#),
    )
    .await;
    let streamed = llm_event_payloads(&text).last().unwrap()["response"]["output"].clone();
    let types = |o: &serde_json::Value| -> Vec<String> {
        o.as_array()
            .unwrap()
            .iter()
            .map(|i| i["type"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(types(&json["output"]), vec!["function_call"]);
    assert_eq!(types(&streamed), types(&json["output"]));
}

#[actix_web::test]
async fn llm_completions_refusal_counts_no_text_tokens() {
    let (_, body) = llm_post(
        "/llm/v1/completions",
        r#"{"prompt":"hi","httpcan":{"error":"content_filter"}}"#,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["choices"][0]["text"], "");
    assert_eq!(v["usage"]["completion_tokens"], 0);
}

#[actix_web::test]
async fn llm_anthropic_auto_tools_skips_server_tools() {
    let (_, body) = llm_post(
        "/llm/v1/messages",
        r#"{"model":"auto-tools","messages":[{"role":"user","content":"hi"}],"tools":[{"type":"web_search_20250305","name":"web_search"},{"name":"get_weather","input_schema":{"type":"object"}}]}"#,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let tool = v["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(tool["name"], "get_weather");
}

// ------------------------------------------------- schema-valid tool arguments

#[::core::prelude::v1::test]
fn llm_sample_from_schema_fills_required_fields() {
    use crate::handlers::llm::sample_from_schema;
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "command": { "type": "string" },
            "timeout": { "type": "integer", "minimum": 1 },
            "mode": { "type": "string", "enum": ["fast", "slow"] },
            "tags": { "type": "array", "items": { "type": "string" }, "minItems": 1 },
            "opts": { "type": "object", "properties": { "x": { "type": "boolean" } }, "required": ["x"] },
            "note": { "type": ["string", "null"], "minLength": 6 },
            "optional": { "type": "string" }
        },
        "required": ["command", "timeout", "mode", "tags", "opts", "note"]
    });
    assert_eq!(
        sample_from_schema(&schema),
        serde_json::json!({
            "command": "mock",
            "timeout": 1,
            "mode": "fast",
            "tags": ["mock"],
            "opts": { "x": false },
            "note": "mockxx"
        })
    );
}

#[actix_web::test]
async fn llm_prompt_mock_tool_args_from_declared_schema() {
    let bash =
        r#"{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}"#;

    let (_, body) = llm_post(
        "/llm/v1/chat/completions",
        &format!(
            r##"{{"messages":[{{"role":"user","content":"#mock tool=bash"}}],"tools":[{{"type":"function","function":{{"name":"bash","parameters":{bash}}}}}]}}"##
        ),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        r#"{"command":"mock"}"#
    );

    let (_, body) = llm_post(
        "/llm/v1/messages",
        &format!(
            r##"{{"messages":[{{"role":"user","content":"#mock tool=bash"}}],"tools":[{{"name":"bash","input_schema":{bash}}}]}}"##
        ),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["content"][0]["input"],
        serde_json::json!({ "command": "mock" })
    );

    let (_, body) = llm_post(
        "/llm/v1/responses",
        &format!(
            r##"{{"input":"#mock tool=bash","tools":[{{"type":"function","name":"bash","parameters":{bash}}}]}}"##
        ),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["output"][0]["arguments"], r#"{"command":"mock"}"#);
}

#[actix_web::test]
async fn llm_auto_tools_and_forced_choice_use_schema_args() {
    let (_, body) = llm_post(
        "/llm/v1/chat/completions",
        r#"{"model":"auto-tools","messages":[{"role":"user","content":"hi"}],"tools":[{"type":"function","function":{"name":"get_weather","parameters":{"type":"object","properties":{"city":{"type":"string","default":"SF"}},"required":["city"]}}}]}"#,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        r#"{"city":"SF"}"#
    );
}

#[actix_web::test]
async fn llm_prompt_mock_explicit_args_directive() {
    let (_, body) = llm_post(
        "/llm/v1/chat/completions",
        r##"{"messages":[{"role":"user","content":"#mock tool=bash args={\"command\": \"ls -la\"}"}],"tools":[{"type":"function","function":{"name":"bash","parameters":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}}}]}"##,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        r#"{"command":"ls -la"}"#
    );
}
