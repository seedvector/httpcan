use super::*;
use serde_json::Value;
use std::collections::BTreeMap;

/// Liveness / health probe for orchestrators (httpbin #544).
pub async fn healthz_handler() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(json!({ "status": "ok" })))
}

/// Return all `HTTPCAN_*` environment variables (prefix stripped), sorted by
/// key. Used for instance identification behind load balancers (httpbin #565).
pub async fn tags_handler() -> Result<HttpResponse> {
    let tags: BTreeMap<String, String> = std::env::vars()
        .filter_map(|(k, v)| k.strip_prefix("HTTPCAN_").map(|s| (s.to_string(), v)))
        .collect();
    Ok(HttpResponse::Ok().json(tags))
}

/// Strip a redundant `HTTPCAN_` prefix from a tag name so that
/// `/tags/VERSION` and `/tags/HTTPCAN_VERSION` resolve to the same variable.
/// The keys returned by [`tags_handler`] are already prefix-stripped, so the
/// unprefixed form is canonical; the prefixed form is accepted for convenience.
pub fn tag_key(raw: &str) -> String {
    raw.strip_prefix("HTTPCAN_").unwrap_or(raw).to_string()
}

/// Return a single tag value by name (httpbin #565).
pub async fn tag_value_handler(path: web::Path<String>) -> Result<HttpResponse> {
    let name = tag_key(&path.into_inner());
    match std::env::var(format!("HTTPCAN_{name}")) {
        Ok(value) => {
            let mut map = serde_json::Map::new();
            map.insert(name, Value::String(value));
            Ok(HttpResponse::Ok().json(Value::Object(map)))
        }
        Err(_) => Ok(HttpResponse::NotFound().json(json!({}))),
    }
}
