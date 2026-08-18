//! `POST /llm/v1/chat/completions` (+ silent alias `/llm/chat/completions`)
//! — mock of the OpenAI Chat Completions API.
//!
//! Non-streaming responses carry every field the OpenAPI 2.3.0 spec marks
//! required (`choices[].logprobs` and `message.refusal` included, as null),
//! plus `usage` with the (optional but expected) `*_details` keys. Streaming
//! follows the real SSE protocol: `chat.completion.chunk` frames, no `event:`
//! lines, one id for the whole stream, role delta first, `finish_reason`
//! chunk last, `data: [DONE]` sentinel, and — when
//! `stream_options.include_usage` is set — an extra empty-choices chunk
//! carrying `usage` between the stop chunk and `[DONE]`.

use super::*;
use actix_web::{web, HttpResponse, Result};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;
const DEFAULT_MODEL: &str = "gpt-5.6";
const DEFAULT_CONTENT: &str = "This is a mock chat completion from httpcan. Point any OpenAI-compatible SDK at this base_url, and every request gets this deterministic placeholder response.";

#[derive(Deserialize)]
struct ChatCompletionsRequest {
    model: Option<String>,
    messages: Option<Vec<InMessage>>,
    n: Option<u32>,
    stream: Option<bool>,
    stream_options: Option<StreamOptions>,
    httpcan: Option<HttpcanOverride>,
}

#[derive(Deserialize)]
struct InMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct StreamOptions {
    include_usage: Option<bool>,
}

fn openai_bad_request(message: String) -> HttpResponse {
    HttpResponse::BadRequest().json(json!({
        "error": {
            "message": message,
            "type": "invalid_request_error",
            "param": null,
            "code": null
        }
    }))
}

/// The `usage` object, shared by the non-streaming response and the
/// include_usage stream chunk.
fn usage_json(prompt_tokens: usize, completion_tokens: usize) -> serde_json::Value {
    json!({
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "total_tokens": prompt_tokens + completion_tokens,
        "prompt_tokens_details": null,
        "completion_tokens_details": null
    })
}

/// One `chat.completion.chunk` value with a fixed id/created/model across the
/// whole stream, as the real API emits.
fn stream_chunk(
    id: &str,
    created: i64,
    model: &str,
    choices: serde_json::Value,
) -> serde_json::Value {
    json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": choices
    })
}

fn choice(delta: serde_json::Value, finish_reason: Option<&str>) -> serde_json::Value {
    json!({
        "index": 0,
        "delta": delta,
        "logprobs": null,
        "finish_reason": finish_reason
    })
}

pub async fn chat_completions_handler(body: web::Bytes) -> Result<HttpResponse> {
    let req: ChatCompletionsRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(err) => {
            return Ok(openai_bad_request(format!(
                "Invalid JSON in request body: {err}"
            )))
        }
    };

    let Some(messages) = req.messages else {
        return Ok(openai_bad_request(
            "Missing required parameter: 'messages'.".into(),
        ));
    };

    let model = req
        .model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let n = req.n.unwrap_or(1).clamp(1, 128);

    // Prompt token estimate over the serialized conversation, using the
    // "role: content\n" heuristic.
    let prompt_text = messages
        .iter()
        .map(|m| {
            format!(
                "{}: {}\n",
                m.role.as_deref().unwrap_or(""),
                content_text(m.content.as_ref().unwrap_or(&serde_json::Value::Null))
            )
        })
        .fold(String::new(), |acc, line| acc + &line);
    let prompt_tokens = estimate_tokens(&prompt_text);

    let content = override_content(req.httpcan.as_ref(), DEFAULT_CONTENT).to_string();
    let completion_tokens = estimate_tokens(&content);

    let id = format!("chatcmpl-{}", rand_hex24());
    let created = now_unix();

    if req.stream.unwrap_or(false) {
        return Ok(stream_chat_completion(
            &id,
            created,
            &model,
            &content,
            prompt_tokens,
            completion_tokens,
            req.stream_options.as_ref().and_then(|o| o.include_usage) == Some(true),
        ));
    }

    let choices: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            json!({
                "index": i,
                "message": {
                    "role": "assistant",
                    "content": content,
                    "refusal": null
                },
                "logprobs": null,
                "finish_reason": "stop"
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": choices,
        "usage": usage_json(prompt_tokens, completion_tokens)
    })))
}

fn sse_frame(chunk: &serde_json::Value) -> web::Bytes {
    web::Bytes::from(format!(
        "data: {}\n\n",
        serde_json::to_string(chunk).expect("chunk serializes")
    ))
}

#[allow(clippy::too_many_arguments)]
fn stream_chat_completion(
    id: &str,
    created: i64,
    model: &str,
    content: &str,
    prompt_tokens: usize,
    completion_tokens: usize,
    include_usage: bool,
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
        // First chunk carries the assistant role.
        yield Ok::<web::Bytes, actix_web::Error>(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
            json!({ "role": "assistant", "content": "" }),
            None
        )]))));

        for word in &words {
            yield Ok(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
                json!({ "content": word }),
                None
            )]))));
            sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;
        }

        // Final chunk: empty delta + finish_reason.
        yield Ok(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
            json!({}),
            Some("stop")
        )]))));

        // When stream_options.include_usage is set, one extra chunk with
        // empty choices and the usage block lands before [DONE].
        if include_usage {
            let mut chunk = stream_chunk(&id, created, &model, json!([]));
            chunk["usage"] = usage_json(prompt_tokens, completion_tokens);
            yield Ok(sse_frame(&chunk));
        }

        yield Ok(web::Bytes::from("data: [DONE]\n\n"));
    };

    sse_response().streaming(stream)
}
