use crate::AppConfig;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde_json::{json, Value};
use std::sync::LazyLock;

/// The OpenAPI specification shipped with this binary. The repository's
/// `static/openapi.json` is the single source of truth, embedded at compile
/// time so the served spec can never describe a different binary than the one
/// serving it. Self-hosters can replace it by placing their own
/// `openapi.json` in the static assets directory (see `ServerConfig::static_dir`).
const EMBEDDED_OPENAPI: &str = include_str!("../../static/openapi.json");

/// Parsed once on first use. Immutable by contract: handlers must clone
/// before mutating (e.g. the `servers` injection below) — mutating the shared
/// value would leak one request's origin into every other response.
static BASE_SPEC: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(EMBEDDED_OPENAPI).expect("embedded openapi.json is valid JSON")
});

/// Number of paths in the embedded spec — used by the homepage's endpoint
/// counters so they can never drift from the served `/openapi.json`.
pub fn spec_path_count() -> usize {
    BASE_SPEC
        .as_object()
        .and_then(|o| o.get("paths"))
        .and_then(|p| p.as_object())
        .map(|p| p.len())
        .unwrap_or(0)
}

// Generate dynamic OpenAPI specification with current server information
pub async fn openapi_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    // Load the base spec: user override file from the static assets dir if
    // present, otherwise the compile-time embedded copy.
    let mut openapi: Value = match std::fs::read_to_string(config.static_path.join("openapi.json"))
    {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(spec) => spec,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError().json(json!({
                    "error": "Failed to parse OpenAPI specification"
                })));
            }
        },
        Err(_) => BASE_SPEC.clone(),
    };

    // Handle servers array based on configuration
    if config.add_current_server {
        // Get current server information from request
        let connection_info = req.connection_info();
        let scheme = connection_info.scheme();
        let host = connection_info.host();
        let current_server_url = format!("{}://{}", scheme, host);

        // Get existing servers array from the OpenAPI spec
        let mut servers_array = Vec::new();

        // Add current server as the first element
        servers_array.push(json!({
            "url": current_server_url,
            "description": "Current server"
        }));

        // Add existing servers from the original OpenAPI spec
        if let Some(existing_servers) = openapi.get("servers").and_then(|s| s.as_array()) {
            for server in existing_servers {
                // Skip if it's the same as current server URL to avoid duplicates
                if let Some(url) = server.get("url").and_then(|u| u.as_str()) {
                    if url != current_server_url {
                        servers_array.push(server.clone());
                    }
                }
            }
        }

        // Update the servers field
        if let Some(obj) = openapi.as_object_mut() {
            obj.insert("servers".to_string(), json!(servers_array));
        }
    }
    // If add_current_server is false, keep the original servers array unchanged

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(openapi))
}

/// `/.well-known/api-catalog` — an RFC 9727 API catalog that lets automated
/// clients and agents discover HTTPCan's API. Published as a Linkset
/// (application/linkset+json) carrying the RFC 9727 profile; the single entry's
/// anchor is the API root, with `service-desc` (OpenAPI spec), `service-doc`
/// (homepage), and `status` (health probe) link relations from RFC 8631. URLs
/// are resolved against the request origin so every instance advertises its own
/// endpoints (mirroring `openapi_handler` and `root_handler`).
pub async fn api_catalog_handler(req: HttpRequest) -> Result<HttpResponse> {
    let connection_info = req.connection_info();
    let base = format!("{}://{}", connection_info.scheme(), connection_info.host());

    let catalog = json!({
        "linkset": [
            {
                "anchor": format!("{base}/"),
                "service-desc": [
                    { "href": format!("{base}/openapi.json"), "type": "application/json" }
                ],
                "service-doc": [
                    { "href": format!("{base}/"), "type": "text/html" }
                ],
                "status": [
                    { "href": format!("{base}/healthz"), "type": "application/json" }
                ]
            }
        ]
    });

    // Serialize manually: `.json()` would force Content-Type: application/json,
    // but the RFC-mandated media type is application/linkset+json (+profile).
    let body =
        serde_json::to_string(&catalog).map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .content_type(
            r#"application/linkset+json; profile="https://www.rfc-editor.org/info/rfc9727""#,
        )
        .body(body))
}
