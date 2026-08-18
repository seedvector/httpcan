//! Mock LLM provider APIs: OpenAI-compatible `/llm/v1/chat/completions` and
//! Anthropic-compatible `/llm/v1/messages`, plus `/llm/v1/models`.
//!
//! Design contract: `internal/llm-endpoints-design.md` (P0 scope). The
//! visible route tree is `/llm/v1/*`; `/llm/chat/completions` is registered
//! as an undocumented alias because the OpenAI SDK appends only
//! `/chat/completions` to `base_url` (the `/v1` lives in OpenAI's default
//! base, not in the append), so `base_url=<origin>/llm` works for both SDK
//! families:
//!
//! - OpenAI SDK:    `base_url=<origin>/llm/v1` (or `/llm` via the alias)
//! - Anthropic SDK: `base_url=<origin>/llm` (appends `/v1/messages`)
//!
//! No authentication is performed — any API key is accepted. Requests are
//! lenient (unknown fields ignored, `model` echoed back verbatim). Response
//! shapes carry every field the official OpenAPI specs mark as required,
//! with optional-but-common fields (`usage`) included.

use crate::AppConfig;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub mod anthropic;
pub mod models;
pub mod openai;
pub mod responses;

pub use anthropic::*;
pub use models::*;
pub use openai::*;
pub use responses::*;

/// Default model echoed for OpenAI-family requests that omit `model`
/// (current flagship per models.dev).
pub(crate) const OPENAI_DEFAULT_MODEL: &str = "gpt-5.6";

/// One named SSE event frame, as the Anthropic and Responses-API stream
/// protocols transport them (`event: <type>` + `data:` payload).
pub(crate) fn event_frame(event: &str, data: &serde_json::Value) -> web::Bytes {
    web::Bytes::from(format!(
        "event: {event}\ndata: {}\n\n",
        serde_json::to_string(data).expect("event serializes")
    ))
}

/// Delay between streamed chunks, matching the cadence real providers use
/// for a mock of this size (50ms).
pub(crate) const STREAM_CHUNK_DELAY_MS: u64 = 50;

pub(crate) fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// 24 lowercase hex chars — the id tail length shared by the real providers
/// (`chatcmpl-…`/`msg_…`), kept regex-checkable.
pub(crate) fn rand_hex24() -> String {
    Uuid::new_v4().simple().to_string()[..24].to_string()
}

/// Rough token estimate (`ceil(len/4)` chars per token): good enough for a plausible `usage` block, never exact.
pub(crate) fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Request-body override block (`"httpcan": {"content": …}`), passed through
/// SDK `extra_body` support. P0 exposes only `content`.
#[derive(Deserialize)]
pub(crate) struct HttpcanOverride {
    pub content: Option<String>,
}

/// Effective output text: the override when set and non-empty, else default.
pub(crate) fn override_content<'a>(ov: Option<&'a HttpcanOverride>, default: &'a str) -> &'a str {
    ov.and_then(|o| o.content.as_deref())
        .filter(|c| !c.is_empty())
        .unwrap_or(default)
}

/// Extract plain text from a message `content` value that may be a plain
/// string or an array of typed parts (only `{"type": "text"}` parts carry
/// text; images/tool parts contribute nothing).
pub(crate) fn content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|part| {
                let obj = part.as_object()?;
                if obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                    obj.get("text").and_then(|t| t.as_str()).map(str::to_string)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// SSE response headers shared by every streaming LLM endpoint. Unlike the
/// legacy `/sse` endpoint, LLM streams never emit `event:` lines in the
/// OpenAI family (real OpenAI SSE has none) and keep one id for all chunks.
pub(crate) fn sse_response() -> actix_web::HttpResponseBuilder {
    let mut builder = HttpResponse::Ok();
    builder.content_type("text/event-stream");
    builder.insert_header(("Cache-Control", "no-cache"));
    builder.insert_header(("Connection", "keep-alive"));
    builder
}

/// `GET /llm` — self-describing index: endpoints, base_url guidance for both
/// SDK families, and the `httpcan` override contract.
pub async fn llm_index_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let base = crate::handlers::utils::resolved_base(&req, &config);
    Ok(HttpResponse::Ok().json(json!({
        "service": "httpcan LLM mock",
        "description": "Mock OpenAI- and Anthropic-compatible LLM APIs. No auth: any API key works.",
        "base_url": {
            "openai_sdk": format!("{base}/llm/v1"),
            "openai_sdk_alias": format!("{base}/llm"),
            "anthropic_sdk": format!("{base}/llm"),
        },
        "endpoints": {
            "chat_completions": {
                "method": "POST",
                "path": "/llm/v1/chat/completions",
                "alias": "/llm/chat/completions",
                "stream": "stream: true (SSE, chat.completion.chunk frames, [DONE] sentinel)"
            },
            "messages": {
                "method": "POST",
                "path": "/llm/v1/messages",
                "stream": "stream: true (SSE, Anthropic event protocol)"
            },
            "models": {
                "method": "GET",
                "path": "/llm/v1/models",
                "note": "OpenAI shape by default; Anthropic shape when an anthropic-version header is present"
            },
            "model_detail": {
                "method": "GET",
                "path": "/llm/v1/models/{model}",
                "note": "Always 200: any model id is echoed back as existing"
            },
        },
        "response_override": {
            "body_field": "httpcan",
            "example": { "httpcan": { "content": "custom reply text" } },
            "note": "Pass via extra_body in SDKs that reject unknown fields."
        }
    })))
}

/// 405 body for POST-only OpenAI-family endpoints, in the OpenAI error shape.
pub async fn openai_method_not_allowed() -> Result<HttpResponse> {
    Ok(HttpResponse::MethodNotAllowed()
        .insert_header(("Allow", "POST"))
        .json(json!({
            "error": {
                "message": "Only POST is allowed on this endpoint.",
                "type": "invalid_request_error",
                "param": null,
                "code": null
            }
        })))
}

/// 405 body for POST-only Anthropic-family endpoints, in the Anthropic error
/// shape.
pub async fn anthropic_method_not_allowed() -> Result<HttpResponse> {
    Ok(HttpResponse::MethodNotAllowed()
        .insert_header(("Allow", "POST"))
        .json(json!({
            "type": "error",
            "error": { "type": "invalid_request_error", "message": "Only POST is allowed on this endpoint." }
        })))
}
