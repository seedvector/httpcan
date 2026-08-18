//! `POST /llm/v1/responses` (+ silent alias `/llm/responses`) — mock of the
//! OpenAI Responses API.
//!
//! Non-streaming responses carry every field the OpenAPI 2.3.0 spec marks
//! required for the `Response` object (`temperature`, `top_p`, `metadata`,
//! `tool_choice`, `incomplete_details`, …), plus the top-level
//! `output_text` convenience field and the `ResponseUsage` block with both
//! `*_details` sub-objects.
//!
//! Streaming follows the real event protocol: **named SSE events** (`event:`
//! lines, unlike chat-completions streams), a `sequence_number` on every
//! event counting from 0, and no `[DONE]` sentinel (the stream ends with
//! `response.completed`). Minimal event sequence for one text output:
//!
//! ```text
//! response.created -> response.in_progress -> response.output_item.added
//!   -> response.content_part.added -> response.output_text.delta*
//!   -> response.output_text.done -> response.content_part.done
//!   -> response.output_item.done -> response.completed
//! ```

use super::*;
use actix_web::{web, HttpResponse, Result};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

const DEFAULT_MODEL: &str = OPENAI_DEFAULT_MODEL;
const DEFAULT_CONTENT: &str = "This is a mock Responses API response from httpcan. Point any OpenAI SDK at this base_url, and every request gets this deterministic placeholder response.";

#[derive(Deserialize)]
struct ResponsesRequest {
    model: Option<String>,
    /// Input: a plain string, or a list of message items whose `content`
    /// may itself be a string or an array of typed parts.
    input: Option<serde_json::Value>,
    instructions: Option<String>,
    stream: Option<bool>,
    httpcan: Option<HttpcanOverride>,
}

/// The `ResponseUsage` object: five required fields including both detail
/// sub-objects.
fn usage_json(input_tokens: usize, output_tokens: usize) -> serde_json::Value {
    json!({
        "input_tokens": input_tokens,
        "input_tokens_details": { "cached_tokens": 0 },
        "output_tokens": output_tokens,
        "output_tokens_details": { "reasoning_tokens": 0 },
        "total_tokens": input_tokens + output_tokens
    })
}

/// The completed output message item (`OutputMessage`, required:
/// id/type/role/content/status) with one `output_text` part
/// (`ResponseOutputText`, required: type/text/annotations).
fn output_message(id: &str, text: &str) -> serde_json::Value {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "status": "completed",
        "content": [
            { "type": "output_text", "text": text, "annotations": [] }
        ]
    })
}

/// Response-object skeleton shared by the non-streaming body and the
/// `response.created`/`response.in_progress` events: every spec-required
/// field present, no output and no usage yet.
fn response_skeleton(
    id: &str,
    created_at: i64,
    model: &str,
    status: &str,
    instructions: Option<&str>,
) -> serde_json::Value {
    json!({
        "id": id,
        "object": "response",
        "created_at": created_at,
        "status": status,
        "error": null,
        "incomplete_details": null,
        "instructions": instructions,
        "max_output_tokens": null,
        "model": model,
        "tools": [],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "metadata": {},
        "temperature": 1.0,
        "top_p": 1.0,
        "output": []
    })
}

/// Extract text from a Responses-API `input` value: string, or a list of
/// message items (`{role, content}` with string or typed-part-array
/// content), or bare text parts (`{type, text}`). Falls back to the
/// serialized JSON for anything else.
fn input_text(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(input_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Object(obj) => {
            if let Some(content) = obj.get("content") {
                let text = input_text(content);
                if !text.is_empty() {
                    return match obj.get("role").and_then(|r| r.as_str()) {
                        Some(role) => format!("{role}: {text}"),
                        None => text,
                    };
                }
            }
            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                return text.to_string();
            }
            serde_json::to_string(input).unwrap_or_default()
        }
        _ => serde_json::to_string(input).unwrap_or_default(),
    }
}

pub async fn responses_handler(body: web::Bytes) -> Result<HttpResponse> {
    let req: ResponsesRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(err) => {
            return Ok(openai_bad_request(format!(
                "Invalid JSON in request body: {err}"
            )))
        }
    };

    let model = req
        .model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let input_tokens = estimate_tokens(&input_text(
        req.input.as_ref().unwrap_or(&serde_json::Value::Null),
    ));

    let content = override_content(req.httpcan.as_ref(), DEFAULT_CONTENT).to_string();
    let output_tokens = estimate_tokens(&content);

    let id = format!("resp_{}", rand_hex24());
    let created_at = now_unix();

    if req.stream.unwrap_or(false) {
        return Ok(stream_response(
            &id,
            created_at,
            &model,
            &content,
            req.instructions.as_deref(),
            input_tokens,
            output_tokens,
        ));
    }

    let message_id = format!("msg_{}", rand_hex24());
    let mut response = response_skeleton(
        &id,
        created_at,
        &model,
        "completed",
        req.instructions.as_deref(),
    );
    response["output"] = json!([output_message(&message_id, &content)]);
    response["output_text"] = json!(&content);
    response["usage"] = usage_json(input_tokens, output_tokens);
    response["completed_at"] = json!(now_unix());

    Ok(HttpResponse::Ok().json(response))
}

fn stream_response(
    id: &str,
    created_at: i64,
    model: &str,
    content: &str,
    instructions: Option<&str>,
    input_tokens: usize,
    output_tokens: usize,
) -> HttpResponse {
    let id = id.to_string();
    let model = model.to_string();
    let content = content.to_string();
    let instructions = instructions.map(str::to_string);
    let message_id = format!("msg_{}", rand_hex24());
    let words: Vec<String> = content
        .split_whitespace()
        .enumerate()
        .map(|(i, w)| {
            if i == 0 {
                w.to_string()
            } else {
                format!(" {w}")
            }
        })
        .collect();

    let stream = async_stream::stream! {
        // Every event carries an incrementing sequence_number, starting at 0.
        let mut seq: u64 = 0;
        let mut framed = move |event: &str, mut data: serde_json::Value| -> web::Bytes {
            data["sequence_number"] = json!(seq);
            seq += 1;
            event_frame(event, &data)
        };

        let skeleton = response_skeleton(&id, created_at, &model, "in_progress", instructions.as_deref());
        yield Ok::<web::Bytes, actix_web::Error>(framed("response.created", json!({
            "type": "response.created",
            "response": skeleton
        })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok::<web::Bytes, actix_web::Error>(framed("response.in_progress", json!({
            "type": "response.in_progress",
            "response": skeleton
        })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_item.added", json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": output_message(&message_id, "")
        })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok::<web::Bytes, actix_web::Error>(framed("response.content_part.added", json!({
            "type": "response.content_part.added",
            "item_id": message_id,
            "output_index": 0,
            "content_index": 0,
            "part": { "type": "output_text", "text": "", "annotations": [] }
        })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        for word in &words {
            yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_text.delta", json!({
                "type": "response.output_text.delta",
                "item_id": message_id,
                "output_index": 0,
                "content_index": 0,
                "delta": word,
                "logprobs": null
            })));
            sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;
        }

        yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_text.done", json!({
            "type": "response.output_text.done",
            "item_id": message_id,
            "output_index": 0,
            "content_index": 0,
            "text": &content,
            "logprobs": null
        })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok::<web::Bytes, actix_web::Error>(framed("response.content_part.done", json!({
            "type": "response.content_part.done",
            "item_id": message_id,
            "output_index": 0,
            "content_index": 0,
            "part": { "type": "output_text", "text": &content, "annotations": [] }
        })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_item.done", json!({
            "type": "response.output_item.done",
            "output_index": 0,
            "item": output_message(&message_id, &content)
        })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        let mut final_response = response_skeleton(&id, created_at, &model, "completed", instructions.as_deref());
        final_response["output"] = json!([output_message(&message_id, &content)]);
        final_response["output_text"] = json!(&content);
        final_response["usage"] = usage_json(input_tokens, output_tokens);
        final_response["completed_at"] = json!(now_unix());
        yield Ok::<web::Bytes, actix_web::Error>(framed("response.completed", json!({
            "type": "response.completed",
            "response": final_response
        })));
    };

    sse_response().streaming(stream)
}
