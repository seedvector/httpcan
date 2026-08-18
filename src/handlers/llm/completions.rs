//! `POST /llm/v1/completions` (+ silent alias `/llm/completions`) — mock of
//! the OpenAI legacy Completions API (text in, text out; no messages array).
//!
//! Non-streaming responses carry every spec-required field (`choices[].`
//! `logprobs` included as null) plus `usage` with the `*_details` keys.
//! `echo: true` prepends the prompt to the completion text, as the real API
//! does. Streaming emits `text_completion` chunks — unlike chat completions
//! there is no role first-chunk — then a stop chunk, then `[DONE]`, with an
//! extra empty-choices usage chunk when `stream_options.include_usage` is
//! set.

use super::*;
use actix_web::{web, HttpResponse, Result};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

const DEFAULT_MODEL: &str = OPENAI_DEFAULT_MODEL;
const DEFAULT_CONTENT: &str = "This is a mock completion response from httpcan. Your prompt was received and this deterministic placeholder text is returned.";

#[derive(Deserialize)]
struct CompletionsRequest {
    model: Option<String>,
    /// string, array of strings, or array of token numbers; joined for the
    /// token estimate.
    prompt: Option<serde_json::Value>,
    n: Option<u32>,
    stream: Option<bool>,
    stream_options: Option<IncludeUsageOnly>,
    echo: Option<bool>,
    httpcan: Option<HttpcanOverride>,
}

#[derive(Deserialize)]
struct IncludeUsageOnly {
    include_usage: Option<bool>,
}

/// Extract prompt text from the legacy `prompt` value: string, or a list of
/// strings / token numbers / token arrays.
fn prompt_text(prompt: &serde_json::Value) -> String {
    match prompt {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Array(tokens) => Some(
                    tokens
                        .iter()
                        .filter_map(|t| match t {
                            serde_json::Value::Number(n) => Some(n.to_string()),
                            serde_json::Value::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(""),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// One `text_completion` chunk with a fixed id/created/model across the
/// stream. Legacy chunks have no role delta and no message wrapper.
fn stream_chunk(
    id: &str,
    created: i64,
    model: &str,
    choices: serde_json::Value,
) -> serde_json::Value {
    json!({
        "id": id,
        "object": "text_completion",
        "created": created,
        "model": model,
        "choices": choices
    })
}

fn choice(text: &str, finish_reason: Option<&str>) -> serde_json::Value {
    json!({
        "text": text,
        "index": 0,
        "logprobs": null,
        "finish_reason": finish_reason
    })
}

pub async fn completions_handler(body: web::Bytes) -> Result<HttpResponse> {
    let req: CompletionsRequest = match serde_json::from_slice(&body) {
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
    let n = req.n.unwrap_or(1).clamp(1, 128);
    let echo = req.echo.unwrap_or(false);

    let prompt = prompt_text(req.prompt.as_ref().unwrap_or(&serde_json::Value::Null));
    let prompt_tokens = estimate_tokens(&prompt);

    // With echo, the returned text starts with the prompt itself.
    let mut text = override_content(req.httpcan.as_ref(), DEFAULT_CONTENT).to_string();
    if echo && !prompt.is_empty() {
        text = format!("{prompt}{text}");
    }
    let completion_tokens = estimate_tokens(&text);

    let id = format!("cmpl-{}", rand_hex24());
    let created = now_unix();

    if req.stream.unwrap_or(false) {
        return Ok(stream_completion(
            &id,
            created,
            &model,
            &text,
            prompt_tokens,
            completion_tokens,
            req.stream_options.as_ref().and_then(|o| o.include_usage) == Some(true),
        ));
    }

    let choices: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            json!({
                "text": text,
                "index": i,
                "logprobs": null,
                "finish_reason": "stop"
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "id": id,
        "object": "text_completion",
        "created": created,
        "model": model,
        "choices": choices,
        "usage": usage_json(prompt_tokens, completion_tokens)
    })))
}

#[allow(clippy::too_many_arguments)]
fn stream_completion(
    id: &str,
    created: i64,
    model: &str,
    text: &str,
    prompt_tokens: usize,
    completion_tokens: usize,
    include_usage: bool,
) -> HttpResponse {
    let id = id.to_string();
    let model = model.to_string();
    let words: Vec<String> = text
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
        // Legacy completions have no role chunk: straight into the text.
        for word in &words {
            yield Ok::<web::Bytes, actix_web::Error>(sse_frame(&stream_chunk(
                &id, created, &model, json!([choice(word, None)]),
            )));
            sleep(Duration::from_millis(STREAM_CHUNK_DELAY_MS)).await;
        }

        // Final chunk: empty text + finish_reason.
        yield Ok(sse_frame(&stream_chunk(&id, created, &model, json!([choice("", Some("stop"))]))));

        if include_usage {
            let mut chunk = stream_chunk(&id, created, &model, json!([]));
            chunk["usage"] = usage_json(prompt_tokens, completion_tokens);
            yield Ok(sse_frame(&chunk));
        }

        yield Ok(web::Bytes::from("data: [DONE]\n\n"));
    };

    sse_response().streaming(stream)
}
