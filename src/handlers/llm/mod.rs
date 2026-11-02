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
pub mod completions;
pub mod models;
pub mod openai;
pub mod responses;

pub use anthropic::*;
pub use completions::*;
pub use models::*;
pub use openai::*;
pub use responses::*;

/// Default model echoed for OpenAI-family requests that omit `model`
/// (current flagship per models.dev).
pub(crate) const OPENAI_DEFAULT_MODEL: &str = "gpt-5.6-sol";

/// One named SSE event frame, as the Anthropic and Responses-API stream
/// protocols transport them (`event: <type>` + `data:` payload).
pub(crate) fn event_frame(event: &str, data: &serde_json::Value) -> web::Bytes {
    web::Bytes::from(format!(
        "event: {event}\ndata: {}\n\n",
        serde_json::to_string(data).expect("event serializes")
    ))
}

/// Default delay between streamed chunks, matching the cadence real
/// providers use for a mock of this size (50ms). Overridable per request
/// via `httpcan.stream_chunk_delay_ms`.
pub(crate) const STREAM_CHUNK_DELAY_MS: u64 = 50;

/// Upper bound for `httpcan.delay_ms` (Kong's Mocking Policy uses the same
/// 10s ceiling).
pub(crate) const MAX_DELAY_MS: u64 = 10_000;

/// Upper bound for `httpcan.stream_chunk_delay_ms`.
pub(crate) const MAX_STREAM_CHUNK_DELAY_MS: u64 = 1_000;

/// Upper bound on deltas per stream: longer content gets several words per
/// delta so any stream finishes in bounded time (httpbun's scheme).
pub(crate) const MAX_STREAM_WORDS: usize = 600;

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

/// Request-body override block (`"httpcan": {…}`), passed through SDK
/// `extra_body` support. The provider-neutral mock script: every handler
/// resolves it into a `ResolvedScript`, then encodes it in its own wire
/// format (`internal/llm-mock-advanced-design.md` §2).
#[derive(Deserialize, Clone, Default)]
pub(crate) struct HttpcanOverride {
    /// Assistant text. May be combined with `tool_calls`.
    pub content: Option<String>,
    /// In-band refusal text (soft `content_filter`): OpenAI `message.refusal`,
    /// Anthropic `stop_reason: "refusal"`, Responses `incomplete_details`.
    pub refusal: Option<String>,
    /// Tool calls to answer with, `{id?, name, arguments}`.
    pub tool_calls: Option<Vec<MockToolCall>>,
    /// Explicit finish/stop reason override; derived per family otherwise.
    pub stop_reason: Option<String>,
    /// Deterministic usage pinning; unset fields fall back to the estimate.
    pub usage: Option<MockUsage>,
    /// Error injection (`rate_limit` / `overloaded` / `context_length` /
    /// `invalid_api_key` / `content_filter`).
    pub error: Option<MockError>,
    /// Pre-response (TTFB) delay, clamped to `MAX_DELAY_MS`.
    pub delay_ms: Option<u64>,
    /// Inter-delta delay for streams, clamped to `MAX_STREAM_CHUNK_DELAY_MS`.
    pub stream_chunk_delay_ms: Option<u64>,
    /// Multi-turn script, advanced by the number of tool turns in the
    /// request — batches of tool results, parallel results counting once
    /// (clamped to the last step). Step fields override the top-level ones.
    pub sequence: Option<Vec<HttpcanOverride>>,
}

/// One mocked tool call in the provider-neutral script. Encoded per family:
/// `message.tool_calls[].function` (chat), `content[]{type:"tool_use"}`
/// (messages), `output[]{type:"function_call"}` (responses).
#[derive(Deserialize, Clone, serde::Serialize)]
pub(crate) struct MockToolCall {
    pub id: Option<String>,
    pub name: String,
    /// When omitted, generated from the declared tool's JSON schema (every
    /// required field filled with a schema-valid value), else `{}`.
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// A client-executable tool the request declared: its name and the JSON
/// schema of its arguments (`parameters` / `input_schema`).
pub(crate) struct DeclaredTool {
    pub name: String,
    pub schema: serde_json::Value,
}

/// Collect declared tools from a request's `tools` array; `extract` returns
/// `(name, schema)` for client tools and `None` for anything to skip.
pub(crate) fn collect_tools<'a>(
    tools: Option<&'a serde_json::Value>,
    extract: impl Fn(&'a serde_json::Value) -> Option<(&'a str, Option<&'a serde_json::Value>)>,
) -> Vec<DeclaredTool> {
    tools
        .and_then(|t| t.as_array())
        .into_iter()
        .flatten()
        .filter_map(extract)
        .map(|(name, schema)| DeclaredTool {
            name: name.to_string(),
            schema: schema.cloned().unwrap_or(serde_json::Value::Null),
        })
        .collect()
}

/// Maximum schema nesting followed when sampling arguments, so recursive or
/// pathological schemas can't blow the stack.
const MAX_SCHEMA_DEPTH: usize = 8;

/// A minimal value that validates against `schema`: honors `const`,
/// `default`, `examples`, `enum`, then the type — objects get their
/// `required` properties only, arrays `minItems` elements, strings
/// `minLength` (and common `format`s), numbers their lower bound.
/// `anyOf`/`oneOf` take the first branch; `allOf` merges object branches.
/// Unresolvable `$ref`s and `pattern` constraints are not followed.
pub(crate) fn sample_from_schema(schema: &serde_json::Value) -> serde_json::Value {
    sample_at(schema, 0)
}

fn sample_at(schema: &serde_json::Value, depth: usize) -> serde_json::Value {
    use serde_json::Value;
    let Some(obj) = schema.as_object() else {
        return Value::Null;
    };
    if depth > MAX_SCHEMA_DEPTH {
        return Value::Null;
    }
    for key in ["const", "default"] {
        if let Some(v) = obj.get(key) {
            return v.clone();
        }
    }
    for key in ["examples", "enum"] {
        if let Some(v) = obj
            .get(key)
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
        {
            return v.clone();
        }
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(first) = obj
            .get(key)
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
        {
            return sample_at(first, depth + 1);
        }
    }
    if let Some(all) = obj.get("allOf").and_then(|v| v.as_array()) {
        let mut merged = serde_json::Map::new();
        for branch in all {
            match sample_at(branch, depth + 1) {
                Value::Object(m) => merged.extend(m),
                other if all.len() == 1 => return other,
                _ => {}
            }
        }
        return Value::Object(merged);
    }

    // `type` may be a string or a list (`["string","null"]`): first non-null.
    let ty = match obj.get("type") {
        Some(Value::String(t)) => Some(t.as_str()),
        Some(Value::Array(ts)) => ts
            .iter()
            .filter_map(|t| t.as_str())
            .find(|t| *t != "null")
            .or(Some("null")),
        _ if obj.contains_key("properties") => Some("object"),
        _ if obj.contains_key("items") => Some("array"),
        _ => None,
    };
    match ty {
        Some("object") => {
            let props = obj.get("properties").and_then(|p| p.as_object());
            let mut out = serde_json::Map::new();
            for name in obj
                .get("required")
                .and_then(|r| r.as_array())
                .into_iter()
                .flatten()
                .filter_map(|n| n.as_str())
            {
                let prop = props.and_then(|p| p.get(name)).unwrap_or(&Value::Null);
                let v = sample_at(prop, depth + 1);
                // An unconstrained property still needs some value.
                out.insert(
                    name.to_string(),
                    if v.is_null() { json!("mock") } else { v },
                );
            }
            Value::Object(out)
        }
        Some("array") => {
            let min = obj
                .get("minItems")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                .min(16);
            let item = obj.get("items").unwrap_or(&Value::Null);
            Value::Array((0..min).map(|_| sample_at(item, depth + 1)).collect())
        }
        Some("string") => {
            let s = match obj.get("format").and_then(|f| f.as_str()) {
                Some("date-time") => "2026-01-01T00:00:00Z".to_string(),
                Some("date") => "2026-01-01".to_string(),
                Some("time") => "00:00:00Z".to_string(),
                Some("email") => "mock@example.com".to_string(),
                Some("uri") | Some("url") => "https://example.com".to_string(),
                Some("uuid") => "00000000-0000-4000-8000-000000000000".to_string(),
                _ => "mock".to_string(),
            };
            let min = obj.get("minLength").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let max = obj
                .get("maxLength")
                .and_then(|v| v.as_u64())
                .map(|m| m as usize);
            let mut s = if s.len() < min {
                format!("{s}{}", "x".repeat(min - s.len()))
            } else {
                s
            };
            if let Some(max) = max {
                s.truncate(max.max(min));
            }
            json!(s)
        }
        Some(t @ ("integer" | "number")) => {
            let bound = obj
                .get("minimum")
                .and_then(|v| v.as_f64())
                .or_else(|| {
                    obj.get("exclusiveMinimum")
                        .and_then(|v| v.as_f64())
                        .map(|m| m + 1.0)
                })
                .unwrap_or(0.0);
            if t == "integer" {
                json!(bound.ceil() as i64)
            } else {
                json!(bound)
            }
        }
        Some("boolean") => json!(false),
        _ => Value::Null,
    }
}

impl MockToolCall {
    /// The override's id, or a generated one with the family's prefix
    /// (`call_` / `toolu_`).
    pub(crate) fn id_or(&self, prefix: &str) -> String {
        self.id
            .clone()
            .unwrap_or_else(|| format!("{prefix}{}", rand_hex24()))
    }

    /// Arguments as the OpenAI family transports them: a JSON-encoded
    /// string. The Anthropic family uses `input` (object) instead.
    pub(crate) fn arguments_json(&self) -> String {
        if self.arguments.is_null() {
            "{}".to_string()
        } else {
            serde_json::to_string(&self.arguments).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

/// Deterministic usage pinning; each field falls back to the estimate.
#[derive(Deserialize, Clone, Copy)]
pub(crate) struct MockUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

/// Injected error. Every variant has a provider-correct wire shape except
/// `ContentFilter`, which is in-band: a 200 response with a refusal.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MockError {
    RateLimit,
    Overloaded,
    ContextLength,
    InvalidApiKey,
    ContentFilter,
}

/// Refusal text for `error: "content_filter"` when no custom `refusal` is set.
pub(crate) const DEFAULT_REFUSAL: &str = "I can't help with that request.";

/// The effective mock script: sequence step merged over the top-level
/// override, timing knobs clamped.
pub(crate) struct ResolvedScript {
    pub content: Option<String>,
    pub refusal: Option<String>,
    pub tool_calls: Vec<MockToolCall>,
    pub stop_reason: Option<String>,
    pub usage: Option<MockUsage>,
    pub error: Option<MockError>,
    pub delay_ms: u64,
    pub stream_chunk_delay_ms: u64,
}

/// Resolve the override for this request: when a `sequence` is present, the
/// step at `tool_results` (the number of tool turns the client sent back —
/// runs of OpenAI `role:"tool"` messages, user messages carrying Anthropic
/// `tool_result` blocks, runs of Responses `function_call_output` items),
/// clamped to the last step,
/// overrides the top-level fields. This keeps the server stateless: the SDK
/// resends the full conversation every turn, so the count is the turn index.
pub(crate) fn resolve_script(ov: Option<&HttpcanOverride>, tool_results: usize) -> ResolvedScript {
    let Some(ov) = ov else {
        return ResolvedScript {
            content: None,
            refusal: None,
            tool_calls: Vec::new(),
            stop_reason: None,
            usage: None,
            error: None,
            delay_ms: 0,
            stream_chunk_delay_ms: STREAM_CHUNK_DELAY_MS,
        };
    };
    let step = ov
        .sequence
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| &s[tool_results.min(s.len() - 1)]);
    ResolvedScript {
        content: step
            .and_then(|s| s.content.clone())
            .or_else(|| ov.content.clone()),
        refusal: step
            .and_then(|s| s.refusal.clone())
            .or_else(|| ov.refusal.clone()),
        tool_calls: step
            .and_then(|s| s.tool_calls.clone())
            .or_else(|| ov.tool_calls.clone())
            .unwrap_or_default(),
        stop_reason: step
            .and_then(|s| s.stop_reason.clone())
            .or_else(|| ov.stop_reason.clone()),
        usage: step.and_then(|s| s.usage).or(ov.usage),
        error: step.and_then(|s| s.error).or(ov.error),
        delay_ms: step
            .and_then(|s| s.delay_ms)
            .or(ov.delay_ms)
            .unwrap_or(0)
            .min(MAX_DELAY_MS),
        stream_chunk_delay_ms: step
            .and_then(|s| s.stream_chunk_delay_ms)
            .or(ov.stream_chunk_delay_ms)
            .unwrap_or(STREAM_CHUNK_DELAY_MS)
            .min(MAX_STREAM_CHUNK_DELAY_MS),
    }
}

impl ResolvedScript {
    /// Effective refusal text: explicit, or the default when `content_filter`
    /// is requested in-band.
    pub(crate) fn refusal_text(&self) -> Option<String> {
        self.refusal.clone().or_else(|| {
            if self.error == Some(MockError::ContentFilter) {
                Some(DEFAULT_REFUSAL.to_string())
            } else {
                None
            }
        })
    }

    /// Whether a hard (error-body) injection applies; `content_filter` is
    /// in-band and excluded.
    pub(crate) fn hard_error(&self) -> Option<MockError> {
        self.error.filter(|e| *e != MockError::ContentFilter)
    }
}

/// When `tool_choice` forces a tool call, returns `Some(explicit_name)`:
/// the name to call when the choice names one, `None` to fall back to the
/// first declared tool. Covers both dialects: string `"required"`, OpenAI
/// `{"type":"function","function":{"name":…}}`, Responses
/// `{"type":"function","name":…}`, Anthropic `{"type":"any"}` and
/// `{"type":"tool","name":…}`.
pub(crate) fn choice_forces_tool(choice: Option<&serde_json::Value>) -> Option<Option<String>> {
    match choice? {
        serde_json::Value::String(s) if s == "required" => Some(None),
        serde_json::Value::Object(obj) => match obj.get("type").and_then(|t| t.as_str()) {
            Some("function") => Some(
                obj.get("function")
                    .and_then(|f| f.get("name"))
                    .or_else(|| obj.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            ),
            Some("tool") | Some("any") => {
                Some(obj.get("name").and_then(|v| v.as_str()).map(str::to_string))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Reserved model id that flips the mock into an automatic tool loop: with
/// `tools` declared it calls the first one (empty arguments), and once the
/// client sends tool results it answers by echoing the last one. GUI clients
/// (Cherry Studio, LobeChat, Open WebUI, …) can't inject an `httpcan` block
/// or force `tool_choice` — the model name is the one knob they all expose.
pub(crate) const AUTO_TOOLS_MODEL: &str = "auto-tools";

pub(crate) fn is_auto_tools(model: &str) -> bool {
    model.eq_ignore_ascii_case(AUTO_TOOLS_MODEL)
}

/// Second-turn reply for the auto tool loop: echo the last tool result so
/// the round-trip is visible in chat UIs.
pub(crate) fn tool_echo_content(last_result: Option<&str>) -> String {
    format!(
        "Tool result: {}",
        last_result.filter(|s| !s.is_empty()).unwrap_or("(empty)")
    )
}

/// Prompt-driven mock directives, for GUI clients (Cherry Studio, LobeChat,
/// Open WebUI, …) that can't send request-body fields: the user types them
/// into the chat box. A `#mock` token in the **last user message** turns the
/// rest of that line into space-separated `key=value` directives:
///
/// - `error=rate_limit|overloaded|context_length|invalid_api_key|content_filter`
/// - `delay=<ms>` (TTFB, clamped to 10s)
/// - `usage=<prompt>x<completion>` (deterministic token counts)
/// - `tool=<name>` (answer with one tool call; arguments sampled from the
///   declared tool's schema, else `{}`)
/// - `args=<json object>` (explicit arguments for `tool=`; takes the rest of
///   the line, so it goes last)
///
/// Unknown tokens are ignored (lenient, like the rest of the server); a
/// `#mock` with no recognized token changes nothing. Only the last user
/// message is scanned, and only while it is the live turn (no tool results
/// after it), so neither history nor later rounds of a tool loop
/// re-trigger a directive, and normal
/// conversation that merely mentions `#mock`-prefixed words (e.g.
/// `#mockery`) doesn't match — the sigil must be its own token.
pub(crate) fn prompt_directives(text: &str) -> Option<HttpcanOverride> {
    // Find `#mock` as a standalone token: start-of-text or after
    // whitespace, followed by whitespace or end-of-text.
    let bytes = text.as_bytes();
    let mut idx = None;
    let mut i = 0;
    while let Some(pos) = text[i..].find("#mock") {
        let at = i + pos;
        let after = at + 5;
        let boundary_before = at == 0 || bytes[at - 1].is_ascii_whitespace();
        let boundary_after = after >= bytes.len() || bytes[after].is_ascii_whitespace();
        if boundary_before && boundary_after {
            idx = Some(at);
            break;
        }
        i = after;
    }
    let at = idx?;
    // Directives live on the same line as the sigil.
    let line = text[at..].lines().next().unwrap_or("");
    // `args=` takes the rest of the line as JSON (it may contain spaces), so
    // it must be the last directive.
    let (line, args) = match line.find(" args=") {
        Some(pos) => (&line[..pos], Some(line[pos + 6..].trim())),
        None => (line, None),
    };
    let args = args
        .and_then(|a| serde_json::from_str::<serde_json::Value>(a).ok())
        .filter(|a| a.is_object());

    let mut ov = HttpcanOverride::default();
    let mut any = false;
    for token in line.split_whitespace().skip(1) {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        match key {
            "error" => {
                let error = match value {
                    "rate_limit" => Some(MockError::RateLimit),
                    "overloaded" => Some(MockError::Overloaded),
                    "context_length" => Some(MockError::ContextLength),
                    "invalid_api_key" => Some(MockError::InvalidApiKey),
                    "content_filter" => Some(MockError::ContentFilter),
                    _ => None,
                };
                if let Some(error) = error {
                    ov.error = Some(error);
                    any = true;
                }
            }
            "delay" => {
                if let Ok(ms) = value.parse::<u64>() {
                    ov.delay_ms = Some(ms);
                    any = true;
                }
            }
            "usage" => {
                // `<prompt>x<completion>`, e.g. `usage=10x5`.
                if let Some((p, c)) = value.split_once('x') {
                    if let (Ok(p), Ok(c)) = (p.parse::<u64>(), c.parse::<u64>()) {
                        ov.usage = Some(MockUsage {
                            prompt_tokens: Some(p),
                            completion_tokens: Some(c),
                        });
                        any = true;
                    }
                }
            }
            "tool" if !value.is_empty() => {
                ov.tool_calls = Some(vec![MockToolCall {
                    id: None,
                    name: value.to_string(),
                    // Unset: filled from the declared tool's schema.
                    arguments: args.clone().unwrap_or(serde_json::Value::Null),
                }]);
                any = true;
            }
            _ => {}
        }
    }
    any.then_some(ov)
}

/// Overlay `top` on `base`: fields set on `top` win. Used to put the
/// request's explicit `httpcan` block above prompt-derived directives —
/// what the caller sent on purpose always beats what the user typed.
pub(crate) fn merge_overrides(
    base: HttpcanOverride,
    top: Option<&HttpcanOverride>,
) -> HttpcanOverride {
    let Some(top) = top else {
        return base;
    };
    HttpcanOverride {
        content: top.content.clone().or(base.content),
        refusal: top.refusal.clone().or(base.refusal),
        tool_calls: top.tool_calls.clone().or(base.tool_calls),
        stop_reason: top.stop_reason.clone().or(base.stop_reason),
        usage: top.usage.or(base.usage),
        error: top.error.or(base.error),
        delay_ms: top.delay_ms.or(base.delay_ms),
        stream_chunk_delay_ms: top.stream_chunk_delay_ms.or(base.stream_chunk_delay_ms),
        sequence: top.sequence.clone().or(base.sequence),
    }
}

/// Provider-neutral facts a handler extracts from its request dialect before
/// planning the answer.
pub(crate) struct PlanInput<'a> {
    /// The explicit request-body `httpcan` block.
    pub httpcan: Option<&'a HttpcanOverride>,
    pub model: &'a str,
    pub tool_choice: Option<&'a serde_json::Value>,
    /// Declared *client* tools (server/built-in tools excluded), in order:
    /// the first one backs forced `tool_choice` and auto-tools synthesis, and
    /// each schema fills the arguments of calls that leave them unset.
    pub tools: Vec<DeclaredTool>,
    /// Completed tool turns: the number of tool-result *batches* the client
    /// sent back (parallel results of one turn count once).
    pub tool_turns: usize,
    /// Text of the last user message, only when it is the live turn (no tool
    /// results after it) — so `#mock` directives fire once, not on every
    /// round of a tool loop.
    pub directive_text: Option<String>,
    /// Last tool result text, for the auto-tools echo reply.
    pub last_tool_result: Option<String>,
    pub default_content: &'a str,
    /// Family prefix for generated tool-call ids (`call_` / `toolu_`).
    pub id_prefix: &'a str,
    pub estimated_prompt_tokens: usize,
}

/// The resolved answer every handler encodes in its own wire format.
pub(crate) struct ResponsePlan {
    pub script: ResolvedScript,
    /// Tool calls with ids assigned once, so every place a call is encoded
    /// (stream events, final objects) carries the same id.
    pub tool_calls: Vec<MockToolCall>,
    /// Assistant text; empty for tool-only answers whose script set no
    /// `content`.
    pub content: String,
    pub refusal: Option<String>,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

/// Merge directives under the `httpcan` block, resolve the script step,
/// synthesize forced / auto-tools calls, pick the content and estimate usage.
pub(crate) fn plan_response(input: PlanInput<'_>) -> ResponsePlan {
    let directives = input
        .directive_text
        .as_deref()
        .and_then(prompt_directives)
        .unwrap_or_default();
    let override_block = merge_overrides(directives, input.httpcan);
    let script = resolve_script(Some(&override_block), input.tool_turns);

    let mut tool_calls = script.tool_calls.clone();
    // Forced tool_choice synthesizes a call when the script has none.
    if tool_calls.is_empty() {
        if let Some(explicit) = choice_forces_tool(input.tool_choice) {
            if let Some(name) = explicit.or_else(|| input.tools.first().map(|t| t.name.clone())) {
                tool_calls.push(MockToolCall {
                    id: None,
                    name,
                    arguments: serde_json::Value::Null,
                });
            }
        }
    }
    // auto-tools: turn 1 calls the first declared tool; later turns echo.
    if tool_calls.is_empty() && input.tool_turns == 0 && is_auto_tools(input.model) {
        if let Some(name) = input.tools.first().map(|t| t.name.clone()) {
            tool_calls.push(MockToolCall {
                id: None,
                name,
                arguments: serde_json::Value::Null,
            });
        }
    }
    for call in &mut tool_calls {
        // Unset arguments: a schema-valid sample of the declared tool, so the
        // client's argument validation accepts the call.
        if call.arguments.is_null() {
            if let Some(tool) = input.tools.iter().find(|t| t.name == call.name) {
                call.arguments = sample_from_schema(&tool.schema);
            }
        }
        if call.id.is_none() {
            call.id = Some(call.id_or(input.id_prefix));
        }
    }

    let content = script.content.clone().unwrap_or_else(|| {
        if !tool_calls.is_empty() {
            String::new()
        } else if input.tool_turns > 0 && is_auto_tools(input.model) {
            tool_echo_content(input.last_tool_result.as_deref())
        } else {
            input.default_content.to_string()
        }
    });
    let refusal = script.refusal_text();
    let prompt_tokens = script
        .usage
        .and_then(|u| u.prompt_tokens)
        .unwrap_or(input.estimated_prompt_tokens as u64) as usize;

    let mut plan = ResponsePlan {
        script,
        tool_calls,
        content,
        refusal,
        prompt_tokens,
        completion_tokens: 0,
    };
    plan.completion_tokens = plan.estimate_completion(&plan.content);
    plan
}

impl ResponsePlan {
    /// Completion tokens for `text`: pinned value wins; otherwise text plus
    /// serialized tool calls, with a refusal contributing no text tokens.
    pub(crate) fn estimate_completion(&self, text: &str) -> usize {
        self.script
            .usage
            .and_then(|u| u.completion_tokens)
            .unwrap_or_else(|| {
                let text = if self.refusal.is_some() {
                    0
                } else {
                    estimate_tokens(text)
                };
                (text + tool_call_tokens(&self.tool_calls)) as u64
            }) as usize
    }

    /// The pre-response (TTFB) delay — applied before any byte goes out,
    /// for error, JSON and streaming responses alike.
    pub(crate) async fn delay(&self) {
        if self.script.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.script.delay_ms)).await;
        }
    }
}

/// Count runs of consecutive items matching `is_result`: one run per tool
/// turn, however many parallel results it carries.
pub(crate) fn count_result_runs<T>(items: &[T], is_result: impl Fn(&T) -> bool) -> usize {
    let mut runs = 0;
    let mut prev = false;
    for item in items {
        let cur = is_result(item);
        if cur && !prev {
            runs += 1;
        }
        prev = cur;
    }
    runs
}

/// Token estimate over the serialized tool calls (0 when there are none),
/// so `usage` stays plausible when a mock answers with tools instead of text.
pub(crate) fn tool_call_tokens(calls: &[MockToolCall]) -> usize {
    if calls.is_empty() {
        return 0;
    }
    estimate_tokens(&serde_json::to_string(calls).unwrap_or_default())
}

/// Split text into stream deltas: one word per delta (leading-space scheme
/// so concatenation reconstructs the text exactly), degenerating to several
/// words per delta past `MAX_STREAM_WORDS` so every stream finishes in
/// bounded time.
pub(crate) fn stream_words(text: &str) -> Vec<String> {
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
    if words.len() <= MAX_STREAM_WORDS {
        return words;
    }
    let per = words.len().div_ceil(MAX_STREAM_WORDS);
    words.chunks(per).map(<[String]>::concat).collect()
}

/// Hard injected error in the OpenAI error envelope (chat / responses /
/// legacy completions share it).
pub(crate) fn openai_error_response(error: MockError) -> HttpResponse {
    use actix_web::http::StatusCode;
    let (status, message, etype, code): (StatusCode, &str, &str, &str) = match error {
        MockError::RateLimit => (
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit reached for requests. Please try again later.",
            "rate_limit_error",
            "rate_limit_exceeded",
        ),
        MockError::Overloaded => (
            StatusCode::SERVICE_UNAVAILABLE,
            "The server is currently overloaded. Please retry later.",
            "server_error",
            "",
        ),
        MockError::ContextLength => (
            StatusCode::BAD_REQUEST,
            "This model's maximum context length is 4096 tokens. However, your request exceeded this limit.",
            "invalid_request_error",
            "context_length_exceeded",
        ),
        MockError::InvalidApiKey => (
            StatusCode::UNAUTHORIZED,
            "Incorrect API key provided.",
            "invalid_request_error",
            "invalid_api_key",
        ),
        // In-band (200 + refusal), never an error body.
        MockError::ContentFilter => unreachable!("content_filter is in-band"),
    };
    let code = if code.is_empty() {
        serde_json::Value::Null
    } else {
        json!(code)
    };
    HttpResponse::build(status).json(json!({
        "error": { "message": message, "type": etype, "param": null, "code": code }
    }))
}

/// Hard injected error in the Anthropic error envelope.
pub(crate) fn anthropic_error_response(error: MockError) -> HttpResponse {
    use actix_web::http::StatusCode;
    let (status, etype, message): (StatusCode, &str, &str) = match error {
        MockError::RateLimit => (
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_error",
            "This request would exceed your organization's rate limit.",
        ),
        MockError::Overloaded => (
            // Anthropic's vendor-specific overload status.
            actix_web::http::StatusCode::from_u16(529).expect("529 is valid"),
            "overloaded_error",
            "Overloaded",
        ),
        MockError::ContextLength => (
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "prompt is too long: 200000 tokens > 190000 maximum",
        ),
        MockError::InvalidApiKey => (
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "invalid x-api-key",
        ),
        // In-band (200 + stop_reason refusal), never an error body.
        MockError::ContentFilter => unreachable!("content_filter is in-band"),
    };
    HttpResponse::build(status).json(json!({
        "type": "error",
        "error": { "type": etype, "message": message }
    }))
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
            "responses": {
                "method": "POST",
                "path": "/llm/v1/responses",
                "alias": "/llm/responses",
                "stream": "stream: true (named SSE events response.created..response.completed, sequence_number, no [DONE])"
            },
            "legacy_completions": {
                "method": "POST",
                "path": "/llm/v1/completions",
                "alias": "/llm/completions",
                "stream": "stream: true (SSE, text_completion frames, [DONE] sentinel)"
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
            "fields": {
                "content": "assistant text (combined with tool_calls when both are set)",
                "tool_calls": [{ "id": "optional", "name": "get_weather", "arguments": { "city": "SF" } }],
                "stop_reason": "explicit finish/stop reason override (derived per family otherwise)",
                "usage": { "prompt_tokens": 10, "completion_tokens": 5 },
                "error": "rate_limit | overloaded | context_length | invalid_api_key | content_filter",
                "refusal": "in-band refusal text (content_filter's soft variant)",
                "delay_ms": "pre-response delay, clamped to 10000",
                "stream_chunk_delay_ms": "inter-delta delay for streams, clamped to 1000",
                "sequence": "multi-turn script; advanced once per tool turn (parallel results count once), last step repeats"
            },
            "example": { "httpcan": { "tool_calls": [ { "name": "get_weather", "arguments": { "city": "SF" } } ] } },
            "note": "Pass via extra_body in SDKs that reject unknown fields. tool_choice forced (required/any/tool) synthesizes a call from the first declared tool when tool_calls is unset. Calls without arguments get a schema-valid sample of the declared tool's parameters (required fields filled), else {}."
        },
        "prompt_directives": {
            "sigil": "#mock",
            "where": "the LAST user message of the request, while no tool results follow it (directives fire once per user turn, not on every tool round); the rest of its line is space-separated key=value directives",
            "keys": {
                "error": "rate_limit | overloaded | context_length | invalid_api_key | content_filter",
                "delay": "milliseconds, e.g. delay=3000",
                "usage": "<prompt>x<completion>, e.g. usage=10x5",
                "tool": "tool name to call, e.g. tool=get_weather; arguments are generated from the declared tool's JSON schema (required fields filled), else {}",
                "args": "explicit JSON-object arguments for tool=; takes the rest of the line, so put it last, e.g. tool=bash args={\"command\": \"ls -la\"}"
            },
            "example_user_message": "#mock error=rate_limit delay=500",
            "note": "For GUI clients (Cherry Studio, LobeChat, Open WebUI, …) that cannot send request-body fields. Explicit httpcan blocks win over prompt directives."
        },
        "auto_tools_model": {
            "model": "auto-tools",
            "behavior": "with tools declared: calls the first declared tool; once the client sends tool results: replies with 'Tool result: <last result>'. Listed in /llm/v1/models so GUI clients can add it with one click."
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
