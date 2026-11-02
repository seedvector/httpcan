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
//!
//! The `httpcan` override block (`internal/llm-mock-advanced-design.md`)
//! adds tool calls (`message.tool_calls`, `finish_reason:"tool_calls"`),
//! error injection, usage pinning, delays and multi-turn sequences on top.

use super::*;
use actix_web::{web, HttpResponse, Result};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

const DEFAULT_MODEL: &str = OPENAI_DEFAULT_MODEL;
const DEFAULT_CONTENT: &str = "This is a mock chat completion from httpcan. Point any OpenAI-compatible SDK at this base_url, and every request gets this deterministic placeholder response.";

#[derive(Deserialize)]
struct ChatCompletionsRequest {
    model: Option<String>,
    messages: Option<Vec<InMessage>>,
    n: Option<u32>,
    stream: Option<bool>,
    stream_options: Option<StreamOptions>,
    tools: Option<serde_json::Value>,
    tool_choice: Option<serde_json::Value>,
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

pub(crate) fn openai_bad_request(message: String) -> HttpResponse {
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
pub(crate) fn usage_json(prompt_tokens: usize, completion_tokens: usize) -> serde_json::Value {
    json!({
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "total_tokens": prompt_tokens + completion_tokens,
        "prompt_tokens_details": null,
        "completion_tokens_details": null
    })
}

/// `message.tool_calls[]` entries: `function.arguments` is the JSON-encoded
/// string, exactly as the real API transports it.
fn tool_calls_json(calls: &[MockToolCall]) -> serde_json::Value {
    json!(calls
        .iter()
        .map(|c| json!({
            "id": c.id_or("call_"),
            "type": "function",
            "function": { "name": c.name, "arguments": c.arguments_json() }
        }))
        .collect::<Vec<_>>())
}

/// Declared function tools (`tools[].function.{name, parameters}`), for
/// forced `tool_choice` / auto-tools synthesis and argument sampling.
fn declared_tools(tools: Option<&serde_json::Value>) -> Vec<DeclaredTool> {
    collect_tools(tools, |t| {
        let f = t.get("function")?;
        Some((f.get("name")?.as_str()?, f.get("parameters")))
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
    let estimated_prompt_tokens = estimate_tokens(&prompt_text);

    let text_of =
        |m: &InMessage| content_text(m.content.as_ref().unwrap_or(&serde_json::Value::Null));
    let is_tool = |m: &InMessage| m.role.as_deref() == Some("tool");
    // Sequence advancement: one step per tool turn (a run of `role:"tool"`
    // messages — parallel results count once).
    let tool_turns = count_result_runs(&messages, is_tool);
    // Prompt `#mock` directives come from the last user message, only while
    // it is the live turn (no tool results after it).
    let directive_text = messages
        .iter()
        .rposition(|m| m.role.as_deref() == Some("user"))
        .filter(|&i| !messages[i + 1..].iter().any(is_tool))
        .map(|i| text_of(&messages[i]));
    let last_tool_result = messages.iter().rev().find(|m| is_tool(m)).map(text_of);
    let answering_tool_results = messages.last().is_some_and(is_tool);

    let plan = plan_response(PlanInput {
        httpcan: req.httpcan.as_ref(),
        model: &model,
        tool_choice: req.tool_choice.as_ref(),
        tools: declared_tools(req.tools.as_ref()),
        tool_turns,
        directive_text,
        last_tool_result,
        answering_tool_results,
        supports_tools: true,
        default_content: DEFAULT_CONTENT,
        id_prefix: "call_",
        estimated_prompt_tokens,
    });
    plan.delay().await;
    if let Some(error) = plan.script.hard_error() {
        return Ok(openai_error_response(error));
    }

    let finish_reason = plan.script.stop_reason.clone().unwrap_or_else(|| {
        match (&plan.tool_calls[..], &plan.refusal) {
            ([], Some(_)) => "content_filter".to_string(),
            ([], None) => "stop".to_string(),
            // A tool-carrying answer finishes with tool_calls even when text
            // accompanies it, as the real API does.
            (_, _) => "tool_calls".to_string(),
        }
    });
    let id = format!("chatcmpl-{}", rand_hex24());
    let created = now_unix();

    if req.stream.unwrap_or(false) {
        return Ok(stream_chat_completion(
            &id,
            created,
            &model,
            &plan.content,
            &plan.refusal,
            &plan.tool_calls,
            &finish_reason,
            plan.prompt_tokens,
            plan.completion_tokens,
            req.stream_options.as_ref().and_then(|o| o.include_usage) == Some(true),
            plan.script.stream_chunk_delay_ms,
        ));
    }

    let choices: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            let mut message = match &plan.refusal {
                Some(text) => json!({ "role": "assistant", "content": null, "refusal": text }),
                // Tool-only answers carry `content: null`, as the real API does.
                None if plan.content.is_empty() && !plan.tool_calls.is_empty() => {
                    json!({ "role": "assistant", "content": null, "refusal": null })
                }
                None => json!({ "role": "assistant", "content": plan.content, "refusal": null }),
            };
            if !plan.tool_calls.is_empty() {
                message["tool_calls"] = tool_calls_json(&plan.tool_calls);
            }
            json!({
                "index": i,
                "message": message,
                "logprobs": null,
                "finish_reason": finish_reason
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": choices,
        "usage": usage_json(plan.prompt_tokens, plan.completion_tokens)
    })))
}

pub(crate) fn sse_frame(chunk: &serde_json::Value) -> web::Bytes {
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
    refusal: &Option<String>,
    tool_calls: &[MockToolCall],
    finish_reason: &str,
    prompt_tokens: usize,
    completion_tokens: usize,
    include_usage: bool,
    chunk_delay_ms: u64,
) -> HttpResponse {
    let id = id.to_string();
    let model = model.to_string();
    let refusal = refusal.clone();
    let tool_calls = tool_calls.to_vec();
    let finish_reason = finish_reason.to_string();
    let words = stream_words(content);

    let stream = async_stream::stream! {
        // First chunk carries the assistant role.
        yield Ok::<web::Bytes, actix_web::Error>(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
            json!({ "role": "assistant", "content": "" }),
            None
        )]))));
        sleep(Duration::from_millis(chunk_delay_ms)).await;

        // Text deltas (skipped for in-band refusals, which carry no content).
        if refusal.is_none() {
            for word in &words {
                yield Ok(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
                    json!({ "content": word }),
                    None
                )]))));
                sleep(Duration::from_millis(chunk_delay_ms)).await;
            }
        }

        // Tool-call deltas: one chunk opening the call (id + name + empty
        // arguments), one carrying the full JSON-encoded arguments — the
        // fragments the SDK concatenates.
        for (index, call) in tool_calls.iter().enumerate() {
            yield Ok(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
                json!({ "tool_calls": [{
                    "index": index,
                    "id": call.id_or("call_"),
                    "type": "function",
                    "function": { "name": call.name, "arguments": "" }
                }] }),
                None
            )]))));
            sleep(Duration::from_millis(chunk_delay_ms)).await;

            yield Ok(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
                json!({ "tool_calls": [{
                    "index": index,
                    "function": { "arguments": call.arguments_json() }
                }] }),
                None
            )]))));
            sleep(Duration::from_millis(chunk_delay_ms)).await;
        }

        // Final chunk: empty delta + finish_reason.
        yield Ok(sse_frame(&stream_chunk(&id, created, &model, json!([choice(
            json!({}),
            Some(finish_reason.as_str())
        )]))));
        sleep(Duration::from_millis(chunk_delay_ms)).await;

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
