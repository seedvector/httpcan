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
//! sentinel (Anthropic doesn't use one). Tool calls stream as
//! `tool_use` blocks (`input_json_delta` partial JSON), per the `httpcan`
//! override block (`internal/llm-mock-advanced-design.md`).

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
    tools: Option<serde_json::Value>,
    tool_choice: Option<serde_json::Value>,
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

/// `content[]{type:"tool_use"}` block: `input` is the arguments object.
fn tool_use_block(call: &MockToolCall) -> serde_json::Value {
    json!({
        "type": "tool_use",
        "id": call.id_or("toolu_"),
        "name": call.name,
        "input": if call.arguments.is_null() { json!({}) } else { call.arguments.clone() }
    })
}

/// Declared *client* tools (`tools[].{name, input_schema}`), for
/// `tool_choice` synthesis (`{"type":"any"}` / `{"type":"tool"}`),
/// auto-tools and argument sampling. Server/built-in tools
/// (`web_search_20250305`, `bash_…`, …) carry a versioned `type` and are
/// executed by the provider, not the client, so they are skipped; client
/// tools have no `type` or `"custom"`.
fn declared_tools(tools: Option<&serde_json::Value>) -> Vec<DeclaredTool> {
    collect_tools(tools, |t| {
        if !matches!(
            t.get("type").and_then(|v| v.as_str()),
            None | Some("custom")
        ) {
            return None;
        }
        Some((t.get("name")?.as_str()?, t.get("input_schema")))
    })
}

/// `tool_result` blocks of a message (empty for non-user messages).
fn tool_results(m: &InMessage) -> impl Iterator<Item = &serde_json::Value> {
    m.content
        .as_ref()
        .filter(|_| m.role.as_deref() == Some("user"))
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
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
    let estimated_input_tokens = estimate_tokens(&prompt_text);

    // Sequence advancement: one step per user message carrying
    // `tool_result` blocks (parallel results of one turn count once).
    let tool_turns = messages
        .iter()
        .filter(|m| tool_results(m).next().is_some())
        .count();
    // Prompt `#mock` directives come from the last user message, only while
    // it is the live turn (it carries no tool results).
    let directive_text = messages
        .iter()
        .rev()
        .find(|m| m.role.as_deref() == Some("user"))
        .filter(|m| tool_results(m).next().is_none())
        .map(|m| content_text(m.content.as_ref().unwrap_or(&serde_json::Value::Null)));
    let last_tool_result = messages
        .iter()
        .rev()
        .find_map(|m| tool_results(m).last())
        .and_then(|p| p.get("content"))
        .map(content_text);

    let plan = plan_response(PlanInput {
        httpcan: req.httpcan.as_ref(),
        model: &model,
        tool_choice: req.tool_choice.as_ref(),
        tools: declared_tools(req.tools.as_ref()),
        tool_turns,
        directive_text,
        last_tool_result,
        default_content: DEFAULT_CONTENT,
        id_prefix: "toolu_",
        estimated_prompt_tokens: estimated_input_tokens,
    });
    plan.delay().await;
    if let Some(error) = plan.script.hard_error() {
        return Ok(anthropic_error_response(error));
    }

    let stop_reason = plan.script.stop_reason.clone().unwrap_or_else(|| {
        match (&plan.tool_calls[..], &plan.refusal) {
            ([], Some(_)) => "refusal".to_string(),
            ([], None) => "end_turn".to_string(),
            (_, _) => "tool_use".to_string(),
        }
    });

    let id = format!("msg_{}", rand_hex24());

    if req.stream.unwrap_or(false) {
        return Ok(stream_messages(
            &id,
            &model,
            &plan.content,
            &plan.refusal,
            &plan.tool_calls,
            &stop_reason,
            plan.prompt_tokens,
            plan.completion_tokens,
            plan.script.stream_chunk_delay_ms,
        ));
    }

    // Refusals replace the text block entirely, as the real API does; a
    // tool-only answer has no text block at all.
    let text = plan.refusal.as_deref().unwrap_or(&plan.content);
    let mut blocks: Vec<serde_json::Value> = Vec::new();
    if !text.is_empty() || plan.tool_calls.is_empty() {
        blocks.push(json!({ "type": "text", "text": text, "citations": null }));
    }
    blocks.extend(plan.tool_calls.iter().map(tool_use_block));

    Ok(HttpResponse::Ok().json(json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "content": blocks,
        "model": model,
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": plan.prompt_tokens,
            "output_tokens": plan.completion_tokens
        }
    })))
}

#[allow(clippy::too_many_arguments)]
fn stream_messages(
    id: &str,
    model: &str,
    content: &str,
    refusal: &Option<String>,
    tool_calls: &[MockToolCall],
    stop_reason: &str,
    input_tokens: usize,
    output_tokens: usize,
    chunk_delay_ms: u64,
) -> HttpResponse {
    let id = id.to_string();
    let model = model.to_string();
    let text = refusal.clone().unwrap_or_else(|| content.to_string());
    let tool_calls = tool_calls.to_vec();
    let stop_reason = stop_reason.to_string();
    let words = stream_words(&text);

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
        sleep(Duration::from_millis(chunk_delay_ms)).await;

        // Text block first (a refusal streams as its text with stop_reason
        // refusal), then one tool_use block per call. A tool-only answer
        // has no text block, so its tool blocks start at index 0.
        let has_text = !text.is_empty() || tool_calls.is_empty();
        if has_text {
            yield Ok(event_frame(
                "content_block_start",
                &json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": { "type": "text", "text": "" }
                })
            ));
            sleep(Duration::from_millis(chunk_delay_ms)).await;

            for word in &words {
                yield Ok(event_frame(
                    "content_block_delta",
                    &json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": { "type": "text_delta", "text": word }
                    })
                ));
                sleep(Duration::from_millis(chunk_delay_ms)).await;
            }

            yield Ok(event_frame("content_block_stop", &json!({ "type": "content_block_stop", "index": 0 })));
            sleep(Duration::from_millis(chunk_delay_ms)).await;
        }

        for (index, call) in tool_calls.iter().enumerate() {
            let index = index + usize::from(has_text);
            yield Ok(event_frame(
                "content_block_start",
                &json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": {
                        "type": "tool_use",
                        "id": call.id_or("toolu_"),
                        "name": call.name,
                        "input": {}
                    }
                })
            ));
            sleep(Duration::from_millis(chunk_delay_ms)).await;

            yield Ok(event_frame(
                "content_block_delta",
                &json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": { "type": "input_json_delta", "partial_json": call.arguments_json() }
                })
            ));
            sleep(Duration::from_millis(chunk_delay_ms)).await;

            yield Ok(event_frame("content_block_stop", &json!({ "type": "content_block_stop", "index": index })));
            sleep(Duration::from_millis(chunk_delay_ms)).await;
        }

        yield Ok(event_frame(
            "message_delta",
            &json!({
                "type": "message_delta",
                "delta": { "stop_reason": stop_reason, "stop_sequence": null },
                "usage": { "output_tokens": output_tokens }
            })
        ));
        sleep(Duration::from_millis(chunk_delay_ms)).await;

        yield Ok(event_frame("message_stop", &json!({ "type": "message_stop" })));
    };

    sse_response().streaming(stream)
}
