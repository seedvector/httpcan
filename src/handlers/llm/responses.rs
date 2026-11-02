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
//!
//! Tool calls (`httpcan` override, `internal/llm-mock-advanced-design.md`)
//! surface as `function_call` output items, streamed via
//! `response.function_call_arguments.delta`/`done` events.

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
    tools: Option<serde_json::Value>,
    tool_choice: Option<serde_json::Value>,
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

/// The completed output message item carrying an in-band refusal part
/// instead of text.
fn refusal_message(id: &str, refusal: &str) -> serde_json::Value {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "status": "completed",
        "content": [
            { "type": "refusal", "refusal": refusal }
        ]
    })
}

/// A `function_call` output item; `arguments` is the JSON-encoded string.
/// `call.id` is assigned once by the planner, so `call_id` is stable across
/// every event and the final response.
fn function_call_item(id: &str, call: &MockToolCall, status: &str) -> serde_json::Value {
    json!({
        "id": id,
        "type": "function_call",
        "status": status,
        "call_id": call.id_or("call_"),
        "name": call.name,
        "arguments": call.arguments_json()
    })
}

/// Declared function tools (`tools[]` with `type:"function"`:
/// `{name, parameters}`), for `tool_choice` synthesis, auto-tools and
/// argument sampling.
fn declared_tools(tools: Option<&serde_json::Value>) -> Vec<DeclaredTool> {
    collect_tools(tools, |t| {
        if t.get("type").and_then(|v| v.as_str()) != Some("function") {
            return None;
        }
        Some((t.get("name")?.as_str()?, t.get("parameters")))
    })
}

fn is_call_output(item: &serde_json::Value) -> bool {
    item.get("type").and_then(|t| t.as_str()) == Some("function_call_output")
}

/// Whether the output carries a message item: a refusal, or non-empty text.
/// Shared by the streaming and non-streaming paths so both produce the same
/// `output` array.
fn has_message_item(content: &str, refusal: &Option<String>) -> bool {
    refusal.is_some() || !content.is_empty()
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
    let estimated_input_tokens = estimate_tokens(&input_text(
        req.input.as_ref().unwrap_or(&serde_json::Value::Null),
    ));

    let items: &[serde_json::Value] = req
        .input
        .as_ref()
        .and_then(|i| i.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    // Sequence advancement: one step per run of `function_call_output`
    // items (parallel results of one turn count once).
    let tool_turns = count_result_runs(items, is_call_output);
    // Prompt `#mock` directives: a plain-string input is the user's prompt;
    // for item lists, the last user-role item while it is the live turn (no
    // function_call_output after it).
    let directive_text = match req.input.as_ref() {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        _ => items
            .iter()
            .rposition(|i| i.get("role").and_then(|r| r.as_str()) == Some("user"))
            .filter(|&at| !items[at + 1..].iter().any(is_call_output))
            .map(|at| input_text(items[at].get("content").unwrap_or(&serde_json::Value::Null))),
    };
    let last_tool_result = items
        .iter()
        .rev()
        .find(|i| is_call_output(i))
        .and_then(|i| i.get("output"))
        .map(|o| o.as_str().map_or_else(|| input_text(o), str::to_string));
    let answering_tool_results = items.last().is_some_and(is_call_output);

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
        estimated_prompt_tokens: estimated_input_tokens,
    });
    plan.delay().await;
    if let Some(error) = plan.script.hard_error() {
        return Ok(openai_error_response(error));
    }
    // The Responses API has no finish_reason: tool calls complete normally,
    // an in-band refusal marks the response incomplete.
    let incomplete = plan.refusal.is_some();
    // Output-item ids, generated once and shared by every event and the
    // final response object.
    let message_id = format!("msg_{}", rand_hex24());
    let item_ids: Vec<String> = plan
        .tool_calls
        .iter()
        .map(|_| format!("fc_{}", rand_hex24()))
        .collect();

    let id = format!("resp_{}", rand_hex24());
    let created_at = now_unix();

    if req.stream.unwrap_or(false) {
        return Ok(stream_response(
            &id,
            created_at,
            &model,
            &plan.content,
            &plan.refusal,
            &plan.tool_calls,
            &message_id,
            &item_ids,
            incomplete,
            req.instructions.as_deref(),
            plan.prompt_tokens,
            plan.completion_tokens,
            plan.script.stream_chunk_delay_ms,
        ));
    }

    let output = final_output(
        &message_id,
        &plan.content,
        &plan.refusal,
        &plan.tool_calls,
        &item_ids,
    );

    let mut response = response_skeleton(
        &id,
        created_at,
        &model,
        if incomplete {
            "incomplete"
        } else {
            "completed"
        },
        req.instructions.as_deref(),
    );
    response["incomplete_details"] = if incomplete {
        json!({ "reason": "content_filter" })
    } else {
        json!(null)
    };
    response["output"] = json!(output);
    response["output_text"] = if plan.refusal.is_some() {
        json!("")
    } else {
        json!(&plan.content)
    };
    response["usage"] = usage_json(plan.prompt_tokens, plan.completion_tokens);
    response["completed_at"] = json!(now_unix());

    Ok(HttpResponse::Ok().json(response))
}

/// The completed `output` array: the message item (when there is one), then
/// one `function_call` item per call.
fn final_output(
    message_id: &str,
    content: &str,
    refusal: &Option<String>,
    tool_calls: &[MockToolCall],
    item_ids: &[String],
) -> Vec<serde_json::Value> {
    let mut output: Vec<serde_json::Value> = Vec::new();
    if let Some(text) = refusal {
        output.push(refusal_message(message_id, text));
    } else if has_message_item(content, refusal) {
        output.push(output_message(message_id, content));
    }
    for (call, item_id) in tool_calls.iter().zip(item_ids) {
        output.push(function_call_item(item_id, call, "completed"));
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn stream_response(
    id: &str,
    created_at: i64,
    model: &str,
    content: &str,
    refusal: &Option<String>,
    tool_calls: &[MockToolCall],
    message_id: &str,
    item_ids: &[String],
    incomplete: bool,
    instructions: Option<&str>,
    input_tokens: usize,
    output_tokens: usize,
    chunk_delay_ms: u64,
) -> HttpResponse {
    let id = id.to_string();
    let model = model.to_string();
    let content = content.to_string();
    let instructions = instructions.map(str::to_string);
    let message_id = message_id.to_string();
    let item_ids = item_ids.to_vec();
    let refusal = refusal.clone();
    let tool_calls = tool_calls.to_vec();
    let has_message = has_message_item(&content, &refusal);
    let words = stream_words(&content);

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
        sleep(Duration::from_millis(chunk_delay_ms)).await;

        yield Ok::<web::Bytes, actix_web::Error>(framed("response.in_progress", json!({
            "type": "response.in_progress",
            "response": skeleton
        })));
        sleep(Duration::from_millis(chunk_delay_ms)).await;

        // Text (or refusal) message item first — omitted for tool-only
        // answers, exactly as in the non-streaming output.
        if has_message {
            if let Some(text) = refusal.as_ref() {
                yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_item.added", json!({
                    "type": "response.output_item.added",
                    "output_index": 0,
                    "item": refusal_message(&message_id, text.as_str())
                })));
                sleep(Duration::from_millis(chunk_delay_ms)).await;
            } else {
                yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_item.added", json!({
                    "type": "response.output_item.added",
                    "output_index": 0,
                    "item": output_message(&message_id, "")
                })));
                sleep(Duration::from_millis(chunk_delay_ms)).await;

                yield Ok::<web::Bytes, actix_web::Error>(framed("response.content_part.added", json!({
                    "type": "response.content_part.added",
                    "item_id": message_id,
                    "output_index": 0,
                    "content_index": 0,
                    "part": { "type": "output_text", "text": "", "annotations": [] }
                })));
                sleep(Duration::from_millis(chunk_delay_ms)).await;

                for word in &words {
                    yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_text.delta", json!({
                        "type": "response.output_text.delta",
                        "item_id": message_id,
                        "output_index": 0,
                        "content_index": 0,
                        "delta": word,
                        "logprobs": null
                    })));
                    sleep(Duration::from_millis(chunk_delay_ms)).await;
                }

                yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_text.done", json!({
                    "type": "response.output_text.done",
                    "item_id": message_id,
                    "output_index": 0,
                    "content_index": 0,
                    "text": &content,
                    "logprobs": null
                })));
                sleep(Duration::from_millis(chunk_delay_ms)).await;

                yield Ok::<web::Bytes, actix_web::Error>(framed("response.content_part.done", json!({
                    "type": "response.content_part.done",
                    "item_id": message_id,
                    "output_index": 0,
                    "content_index": 0,
                    "part": { "type": "output_text", "text": &content, "annotations": [] }
                })));
                sleep(Duration::from_millis(chunk_delay_ms)).await;
            }

            yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_item.done", json!({
                "type": "response.output_item.done",
                "output_index": 0,
                "item": if let Some(text) = refusal.as_ref() {
                    refusal_message(&message_id, text.as_str())
                } else {
                    output_message(&message_id, &content)
                }
            })));
            sleep(Duration::from_millis(chunk_delay_ms)).await;
        }

        // Function-call items after the message item.
        for (output_index, (call, item_id)) in tool_calls.iter().zip(&item_ids).enumerate() {
            let output_index = output_index + usize::from(has_message);
            let added = function_call_item(item_id, call, "in_progress");
            yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_item.added", json!({
                "type": "response.output_item.added",
                "output_index": output_index,
                "item": added
            })));
            sleep(Duration::from_millis(chunk_delay_ms)).await;

            yield Ok::<web::Bytes, actix_web::Error>(framed("response.function_call_arguments.delta", json!({
                "type": "response.function_call_arguments.delta",
                "item_id": item_id,
                "output_index": output_index,
                "delta": call.arguments_json()
            })));
            sleep(Duration::from_millis(chunk_delay_ms)).await;

            yield Ok::<web::Bytes, actix_web::Error>(framed("response.function_call_arguments.done", json!({
                "type": "response.function_call_arguments.done",
                "item_id": item_id,
                "output_index": output_index,
                "arguments": call.arguments_json()
            })));
            sleep(Duration::from_millis(chunk_delay_ms)).await;

            yield Ok::<web::Bytes, actix_web::Error>(framed("response.output_item.done", json!({
                "type": "response.output_item.done",
                "output_index": output_index,
                "item": function_call_item(item_id, call, "completed")
            })));
            sleep(Duration::from_millis(chunk_delay_ms)).await;
        }

        let mut final_response = response_skeleton(
            &id,
            created_at,
            &model,
            if incomplete { "incomplete" } else { "completed" },
            instructions.as_deref(),
        );
        final_response["incomplete_details"] = if incomplete {
            json!({ "reason": "content_filter" })
        } else {
            json!(null)
        };
        final_response["output"] = json!(final_output(&message_id, &content, &refusal, &tool_calls, &item_ids));
        final_response["output_text"] = if refusal.is_some() { json!("") } else { json!(&content) };
        final_response["usage"] = usage_json(input_tokens, output_tokens);
        final_response["completed_at"] = json!(now_unix());
        yield Ok::<web::Bytes, actix_web::Error>(framed("response.completed", json!({
            "type": "response.completed",
            "response": final_response
        })));
    };

    sse_response().streaming(stream)
}
