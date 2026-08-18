//! `POST /llm/v1/messages` — mock of the Anthropic Messages API.
//!
//! P0 emits the SDK-compatible shape (the one the current anthropic SDK
//! round-trips): text content blocks with
//! `citations`, `stop_reason: "end_turn"`, and the two-field `usage`. The
//! spec-exact `anthropic-strict` dialect with the newer required fields
//! (`stop_details`, `container`, expanded usage) is P1 work
//! (`internal/llm-endpoints-design.md` §5.2).
//!
//! Streaming follows Anthropic's event protocol exactly: `event:` lines,
//! `message_start → content_block_start → content_block_delta* →
//! content_block_stop → message_delta → message_stop`, and no `[DONE]`
//! sentinel (Anthropic doesn't use one).

use super::*;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;
const DEFAULT_MODEL: &str = "claude-sonnet-5";
const DEFAULT_CONTENT: &str = "This is a mock Anthropic messages response from httpcan. Point any Anthropic-compatible SDK at this base_url, and every request gets this deterministic placeholder response.";

#[derive(Deserialize)]
struct MessagesRequest {
    model: Option<String>,
    messages: Option<Vec<InMessage>>,
    max_tokens: Option<u32>,
    stream: Option<bool>,
    /// Top-level system prompt (string or content-block array); counted into
    /// input tokens.
    system: Option<serde_json::Value>,
    httpcan: Option<HttpcanOverride>,
}

#[derive(Deserialize)]
struct InMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

fn anthropic_bad_request(message: String) -> HttpResponse {
    HttpResponse::BadRequest().json(json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": message
        }
    }))
}

pub async fn anthropic_messages_handler(body: web::Bytes) -> Result<HttpResponse> {
    let req: MessagesRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(err) => {
            return Ok(anthropic_bad_request(format!(
                "Invalid JSON in request body: {err}"
            )))
        }
    };

    let Some(messages) = req.messages else {
        return Ok(anthropic_bad_request(
            "Missing required parameter: 'messages'.".into(),
        ));
    };

    let model = req
        .model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    // The spec requires max_tokens from the client; lenient default, like
    // every compatibility provider.
    let _max_tokens = req.max_tokens.unwrap_or(1024);

    let mut prompt_text = String::new();
    if let Some(system) = req.system.as_ref() {
        let sys = content_text(system);
        if !sys.is_empty() {
            prompt_text.push_str(&format!("system: {sys}\n"));
        }
    }
    for m in &messages {
        prompt_text.push_str(&format!(
            "{}: {}\n",
            m.role.as_deref().unwrap_or(""),
            content_text(m.content.as_ref().unwrap_or(&serde_json::Value::Null))
        ));
    }
    let input_tokens = estimate_tokens(&prompt_text);

    let content = override_content(req.httpcan.as_ref(), DEFAULT_CONTENT).to_string();
    let output_tokens = estimate_tokens(&content);

    let id = format!("msg_{}", rand_hex24());

    if req.stream.unwrap_or(false) {
        return Ok(stream_messages(
            &id,
            &model,
            &content,
            input_tokens,
            output_tokens,
        ));
    }

    Ok(HttpResponse::Ok().json(json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "content": [
            { "type": "text", "text": content, "citations": null }
        ],
        "model": model,
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }
    })))
}

fn stream_messages(
    id: &str,
    model: &str,
    content: &str,
    input_tokens: usize,
    output_tokens: usize,
) -> HttpResponse {
    let id = id.to_string();
    let model = model.to_string();
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
        yield Ok::<web::Bytes, actix_web::Error>(event_frame(
            "message_start",
            &json!({
                "type": "message_start",
                "message": {
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                    "model": model,
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": { "input_tokens": input_tokens, "output_tokens": 0 }
                }
            })
        ));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok(event_frame(
            "content_block_start",
            &json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "text", "text": "" }
            })
        ));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        for word in &words {
            yield Ok(event_frame(
                "content_block_delta",
                &json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": { "type": "text_delta", "text": word }
                })
            ));
            sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;
        }

        yield Ok(event_frame("content_block_stop", &json!({ "type": "content_block_stop", "index": 0 })));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok(event_frame(
            "message_delta",
            &json!({
                "type": "message_delta",
                "delta": { "stop_reason": "end_turn", "stop_sequence": null },
                "usage": { "output_tokens": output_tokens }
            })
        ));
        sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;

        yield Ok(event_frame("message_stop", &json!({ "type": "message_stop" })));
    };

    sse_response().streaming(stream)
}
