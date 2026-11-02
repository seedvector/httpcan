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

use super::{ANTHROPIC_DEFAULT_MODEL, AUTO_TOOLS_MODEL, OPENAI_DEFAULT_MODEL};
use crate::AppConfig;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde_json::json;

// The catalog is mock-owned on purpose: real vendor ids go stale with every
// release, and a refreshed list breaks tests that assert on it. The default
// catalog is the family's default model plus the auto-tools model, and it is
// frozen. Deployments that want other ids listed (GUI pickers, vendor-named
// tests) set `--llm-models` / `HTTPCAN_LLM_MODELS`; requests accept any id
// either way.

/// Fixed metadata stamps. Frozen mock values (not any real model's release
/// date or limits), so responses stay deterministic across upgrades.
const OPENAI_MODEL_CREATED: i64 = 1_783_555_200; // 2026-07-09T00:00:00Z
const ANTHROPIC_MODEL_CREATED_AT: &str = "2026-07-24T00:00:00Z";
const ANTHROPIC_MAX_INPUT_TOKENS: i64 = 1_000_000;
const ANTHROPIC_MAX_TOKENS: i64 = 128_000;

/// Catalog ids for one family: the configured list (shared by both
/// families) or the family default, then the auto-tools model, deduplicated.
fn catalog<'a>(configured: &'a [String], default_model: &'a str) -> Vec<&'a str> {
    let mut ids: Vec<&str> = if configured.is_empty() {
        vec![default_model]
    } else {
        configured.iter().map(String::as_str).collect()
    };
    ids.push(AUTO_TOOLS_MODEL);
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}

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

pub async fn models_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    // The reserved auto-tools model rides at the end of both catalogs so
    // GUI clients that fetch the model list (Cherry Studio, …) can add it
    // with one click and exercise tool loops without body control.
    if is_anthropic_client(&req) {
        let ids = catalog(&config.llm_models, ANTHROPIC_DEFAULT_MODEL);
        let data: Vec<serde_json::Value> = ids.iter().map(|id| anthropic_model(id)).collect();
        return Ok(HttpResponse::Ok().json(json!({
            "data": data,
            "first_id": ids.first(),
            "has_more": false,
            "last_id": ids.last()
        })));
    }

    let ids = catalog(&config.llm_models, OPENAI_DEFAULT_MODEL);
    let data: Vec<serde_json::Value> = ids.iter().map(|id| openai_model(id)).collect();
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
