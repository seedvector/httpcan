//! `GET /llm/v1/models` and `GET /llm/v1/models/{model}` — model listing
//! mock.
//!
//! Both SDK families construct the same URL (`/llm/v1/models`): the OpenAI
//! SDK appends `/models` to a base ending in `/v1`, the Anthropic SDK
//! appends `/v1/models`. The two response shapes are incompatible, so the
//! family is picked by sniffing the `anthropic-version` request header,
//! which the Anthropic SDK sends on every request — a stateless
//! discriminator, no query parameter needed.
//!
//! The single-model endpoint always returns 200 and echoes the requested id
//! (fields synthesized on the spot), staying consistent with the chat
//! endpoints' lenient "any model name is valid" semantics: clients that
//! pre-validate a model before sending never see a mismatch.

use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde_json::json;

/// Static catalog per protocol family. A mock has no deployments, so the
/// lists are illustrative cross-sections of each family's current lineup
/// (per models.dev, refreshed 2026-08-18).
const OPENAI_MODELS: &[&str] = &[
    "gpt-5.6",
    "gpt-5.5",
    "gpt-5.5-pro",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
];
const ANTHROPIC_MODELS: &[&str] = &[
    "claude-opus-5",
    "claude-sonnet-5",
    "claude-fable-5",
    "claude-haiku-4-5",
];

/// Fixed `created`/`created_at` stamps — a mock's model list never changes,
/// so stable values keep responses deterministic. Each is the release date
/// of the newest listed model (gpt-5.6: 2026-07-09; claude-opus-5:
/// 2026-07-24), with the 5-series context/output limits from models.dev.
const OPENAI_MODEL_CREATED: i64 = 1_783_555_200;
const ANTHROPIC_MODEL_CREATED_AT: &str = "2026-07-24T00:00:00Z";
const ANTHROPIC_MAX_INPUT_TOKENS: i64 = 1_000_000;
const ANTHROPIC_MAX_TOKENS: i64 = 128_000;

fn is_anthropic_client(req: &HttpRequest) -> bool {
    req.headers().contains_key("anthropic-version")
}

fn openai_model(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "object": "model",
        "created": OPENAI_MODEL_CREATED,
        "owned_by": "httpcan"
    })
}

fn anthropic_model(id: &str) -> serde_json::Value {
    json!({
        "type": "model",
        "id": id,
        "display_name": id,
        "created_at": ANTHROPIC_MODEL_CREATED_AT,
        "capabilities": null,
        "max_input_tokens": ANTHROPIC_MAX_INPUT_TOKENS,
        "max_tokens": ANTHROPIC_MAX_TOKENS
    })
}

pub async fn models_handler(req: HttpRequest) -> Result<HttpResponse> {
    if is_anthropic_client(&req) {
        let data: Vec<serde_json::Value> = ANTHROPIC_MODELS
            .iter()
            .map(|id| anthropic_model(id))
            .collect();
        return Ok(HttpResponse::Ok().json(json!({
            "data": data,
            "first_id": ANTHROPIC_MODELS.first(),
            "has_more": false,
            "last_id": ANTHROPIC_MODELS.last()
        })));
    }

    let data: Vec<serde_json::Value> = OPENAI_MODELS.iter().map(|id| openai_model(id)).collect();
    Ok(HttpResponse::Ok().json(json!({
        "object": "list",
        "data": data
    })))
}

pub async fn model_detail_handler(
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let model = path.into_inner();
    let body = if is_anthropic_client(&req) {
        anthropic_model(&model)
    } else {
        openai_model(&model)
    };
    Ok(HttpResponse::Ok().json(body))
}
