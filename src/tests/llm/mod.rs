//! LLM provider API mocks, one module per handler in `handlers::llm`.
//! Shared SSE/JSON helpers live here.

use super::*;

/// Sorted key set of a JSON object, for exact-shape assertions.
fn llm_sorted_keys(v: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

/// Split an SSE body into its non-empty frames.
fn llm_sse_frames(text: &str) -> Vec<&str> {
    text.split("\n\n").filter(|f| !f.is_empty()).collect()
}

/// Parse the `data:` payload of one SSE frame.
fn llm_frame_data(frame: &str) -> serde_json::Value {
    serde_json::from_str(frame.strip_prefix("data: ").unwrap_or(frame)).unwrap()
}

mod advanced;
mod anthropic;
mod completions;
mod models;
mod openai;
mod responses;
