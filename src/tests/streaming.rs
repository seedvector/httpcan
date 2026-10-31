//! Streaming: /sse /ndjson

use super::*;

/// `/sse?count=3&delay=0&format=simple` streams 3 JSON data frames, then the
/// handler appends an `event: end` completion frame and a `data: [DONE]`
/// sentinel (5 total `data:` lines). Verifies content-type and frame shape.
#[actix_web::test]
async fn sse_simple_count_frames() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/sse?count=3&delay=0&format=simple")
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
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    // 3 loop frames each carry a JSON object payload (`data: {`).
    assert_eq!(text.matches("data: {").count(), 3);
    // Plus the `event: end` frame and the `[DONE]` sentinel => 5 `data:` lines.
    assert_eq!(text.matches("data: ").count(), 5);
    assert!(text.contains("event: end"));
    assert!(text.contains("data: [DONE]"));
    // Simple frames report their 1-based event index out of the total.
    assert!(text.contains("Hello from HTTPCan SSE! Event 1/3"));
    assert!(text.contains("Hello from HTTPCan SSE! Event 3/3"));
}

/// `/sse/2?delay=0` honors the path count param: 2 streamed JSON frames plus
/// the standard termination (4 `data:` lines total).
#[actix_web::test]
async fn sse_path_count() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/sse/2?delay=0").to_request();
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
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    assert_eq!(text.matches("data: {").count(), 2);
    assert_eq!(text.matches("data: ").count(), 4);
    assert!(text.contains("data: [DONE]"));
    assert!(text.contains("Hello from HTTPCan SSE! Event 1/2"));
    assert!(text.contains("Hello from HTTPCan SSE! Event 2/2"));
}

/// `/sse?count=2&delay=0&format=openai` streams OpenAI `chat.completion.chunk`
/// frames: each data frame carries a `choices` array and model `httpcan-sse`,
/// and the stream closes with a `finish_reason: "stop"` chunk then `[DONE]`.
#[actix_web::test]
async fn sse_openai_format() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/sse?count=2&delay=0&format=openai")
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
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    // 2 loop chunks + 1 final stop chunk all carry the OpenAI choices shape.
    assert_eq!(text.matches("\"choices\"").count(), 3);
    assert!(text.contains("\"object\":\"chat.completion.chunk\""));
    assert!(text.contains("\"model\":\"httpcan-sse\""));
    assert!(text.contains("\"role\":\"assistant\""));
    // Final completion chunk signals stop, then the [DONE] sentinel.
    assert!(text.contains("\"finish_reason\":\"stop\""));
    assert!(text.contains("data: [DONE]"));
    // OpenAI format emits `event: message` lines (format != simple branch).
    assert!(text.contains("event: message"));
}

/// `/ndjson?count=3&delay=0&format=simple` emits exactly 3 newline-delimited
/// JSON objects with content-type `application/x-ndjson` (no termination sentinel).
#[actix_web::test]
async fn ndjson_simple_count() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/ndjson?count=3&delay=0&format=simple")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-ndjson"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    let objects: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    assert_eq!(
        objects.len(),
        3,
        "exactly 3 NDJSON objects; body was:\n{text}"
    );
    for (i, line) in objects.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON object");
        assert_eq!(v["event"].as_u64(), Some((i + 1) as u64));
        assert!(v["message"]
            .as_str()
            .unwrap()
            .contains("Hello from HTTPCan NDJSON!"));
        assert!(v.get("timestamp").is_some());
    }
}

/// `/ndjson/2?delay=0` honors the path count param: exactly 2 newline-delimited objects.
#[actix_web::test]
async fn ndjson_path_count() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/ndjson/2?delay=0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-ndjson"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    let objects: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    assert_eq!(
        objects.len(),
        2,
        "exactly 2 NDJSON objects; body was:\n{text}"
    );
    // Default format is simple, so each object carries its event index.
    let first: serde_json::Value = serde_json::from_str(objects[0]).expect("valid JSON");
    assert_eq!(first["event"].as_u64(), Some(1));
}

/// `/ndjson?count=1&delay=0&format=ollama` emits a single Ollama-shaped object
/// with `model` (default `llama2`), a string `response`, and `done: true`
/// (the sole event is the last one, so completion metadata is attached).
#[actix_web::test]
async fn ndjson_ollama_format() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/ndjson?count=1&delay=0&format=ollama")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-ndjson"
    );
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf8 body");
    let objects: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    assert_eq!(objects.len(), 1);
    let v: serde_json::Value = serde_json::from_str(objects[0]).expect("valid JSON object");
    assert_eq!(v["model"].as_str(), Some("llama2"));
    assert!(v["response"].is_string());
    assert_eq!(v["done"].as_bool(), Some(true));
    assert!(v.get("created_at").is_some());
    // The done/final event carries ollama context + eval metadata.
    assert!(v.get("context").is_some());
    assert!(v.get("eval_count").is_some());
    assert!(v.get("total_duration").is_some());
}

/// GET /sse/{count}/{delay} exercises sse_path_with_delay_handler:
/// count and delay both taken from the path. 2 events with delay=0.
#[actix_web::test]
async fn sse_count_delay_path_variant() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/sse/2/0").to_request();
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
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf-8");
    // 2 data frames + termination frame + [DONE] sentinel = 4 "data:" lines.
    let data_lines = body.lines().filter(|l| l.starts_with("data:")).count();
    assert_eq!(
        data_lines, 4,
        "expected 4 data: lines (2 events + end + DONE): {body}"
    );
}

/// GET /ndjson/{count}/{delay} exercises ndjson_path_with_delay_handler.
/// Exactly 2 newline-delimited objects.
#[actix_web::test]
async fn ndjson_count_delay_path_variant() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/ndjson/2/0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf-8");
    let objs: Vec<&str> = body.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(objs.len(), 2, "expected exactly 2 NDJSON objects: {body}");
    for o in &objs {
        serde_json::from_str::<serde_json::Value>(o).expect("each line is valid JSON");
    }
}
