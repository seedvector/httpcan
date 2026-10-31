//! OpenAI legacy completions mock (/llm/v1/completions)

use super::*;

#[actix_web::test]
async fn llm_completions_canonical_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"model":"gpt-5.6-sol","prompt":"Hello"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        llm_sorted_keys(&v),
        vec!["choices", "created", "id", "model", "object", "usage"]
    );
    assert!(v["id"].as_str().unwrap().starts_with("cmpl-"));
    assert_eq!(v["object"], "text_completion");
    assert_eq!(v["model"], "gpt-5.6-sol");

    let choice = &v["choices"][0];
    assert_eq!(
        llm_sorted_keys(choice),
        vec!["finish_reason", "index", "logprobs", "text"]
    );
    assert_eq!(choice["finish_reason"], "stop");
    assert!(choice["logprobs"].is_null());
    assert!(choice["text"]
        .as_str()
        .unwrap()
        .starts_with("This is a mock completion response"));

    assert_eq!(v["usage"]["prompt_tokens"], 2); // "Hello" -> ceil(5/4)
    assert_eq!(
        v["usage"]["total_tokens"],
        v["usage"]["prompt_tokens"].as_u64().unwrap()
            + v["usage"]["completion_tokens"].as_u64().unwrap()
    );
    assert_eq!(v["usage"].as_object().unwrap().len(), 5);
}

#[actix_web::test]
async fn llm_completions_prompt_array_echo_and_custom_content() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"prompt":["Say hi","Say hi"],"echo":true,"n":2,"httpcan":{"content":"Done."}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let choices = v["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0]["index"], 0);
    assert_eq!(choices[1]["index"], 1);
    // echo prepends the (joined) prompt to the completion text.
    assert_eq!(choices[0]["text"], "Say hi Say hiDone.");
    // prompt_tokens over "Say hi Say hi" (14 chars) = ceil(14/4) = 4
    assert_eq!(v["usage"]["prompt_tokens"], 4);
    assert_eq!(
        v["usage"]["completion_tokens"],
        crate::handlers::llm::estimate_tokens("Say hi Say hiDone.")
    );
    // No model in request -> default echoed.
    assert_eq!(v["model"], "gpt-5.6-sol");
}

#[actix_web::test]
async fn llm_completions_stream_protocol() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"prompt":"Hello","stream":true,"httpcan":{"content":"word one two"}}"#)
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
    assert!(!text.contains("event:"));

    let frames = llm_sse_frames(&text);
    assert_eq!(frames.last().unwrap(), &"data: [DONE]");
    let chunks: Vec<serde_json::Value> = frames[..frames.len() - 1]
        .iter()
        .map(|f| llm_frame_data(f))
        .collect();

    let id = chunks[0]["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("cmpl-"));
    for chunk in &chunks {
        assert_eq!(chunk["object"], "text_completion");
        assert_eq!(chunk["id"], id.as_str());
        assert!(chunk.get("usage").is_none());
    }
    // No role first-chunk in the legacy protocol: first frame is already text.
    assert!(chunks[0]["choices"][0]["text"].as_str().unwrap() == "word");

    let mut reconstructed = String::new();
    for chunk in &chunks[..chunks.len() - 1] {
        reconstructed.push_str(chunk["choices"][0]["text"].as_str().unwrap());
    }
    assert_eq!(reconstructed, "word one two");

    let last = chunks.last().unwrap();
    assert_eq!(last["choices"][0]["text"], "");
    assert_eq!(last["choices"][0]["finish_reason"], "stop");
}

#[actix_web::test]
async fn llm_completions_stream_include_usage() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(
            r#"{"prompt":"Hi","stream":true,"stream_options":{"include_usage":true},"httpcan":{"content":"a b"}}"#,
        )
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    let frames = llm_sse_frames(&text);
    assert_eq!(frames.last().unwrap(), &"data: [DONE]");
    let usage_frame = llm_frame_data(frames[frames.len() - 2]);
    assert_eq!(usage_frame["choices"].as_array().unwrap().len(), 0);
    assert_eq!(usage_frame["usage"]["prompt_tokens"], 1); // "Hi" -> ceil(2/4)
    let stop_frame = llm_frame_data(frames[frames.len() - 3]);
    assert_eq!(stop_frame["choices"][0]["finish_reason"], "stop");
}

#[actix_web::test]
async fn llm_completions_alias_405_and_errors() {
    let app = test::init_service(create_app(cfg())).await;

    let req = test::TestRequest::post()
        .uri("/llm/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"prompt":"Hello"}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["object"], "text_completion");

    let req = test::TestRequest::get()
        .uri("/llm/v1/completions")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        resp.headers().get("Allow").unwrap().to_str().unwrap(),
        "POST"
    );

    let req = test::TestRequest::post()
        .uri("/llm/v1/completions")
        .insert_header(("content-type", "application/json"))
        .set_payload("not json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
}
