use crate::handlers::utils::get_static_path;
use crate::AppConfig;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde_json::{json, Value};

// Generate dynamic OpenAPI specification with current server information
pub async fn openapi_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let static_path = get_static_path();
    let openapi_path = static_path.join("openapi.json");

    // Read the base OpenAPI specification
    let base_openapi = match std::fs::read_to_string(&openapi_path) {
        Ok(content) => content,
        Err(_) => {
            // Return helpful information when openapi.json is not found
            return Ok(HttpResponse::NotFound().json(json!({
                "info": {
                    "title": "HTTPCan",
                    "version": option_env!("CARGO_PKG_VERSION").unwrap_or("unknown"),
                    "description": "A simple, high‑performance HTTP request & response service built with Rust and Actix Web. Fully compatible with [httpbin.org](https://httpbin.org), with modern streaming and AI‑friendly enhancements."
                },
                "error": "OpenAPI specification not found",
                "message": "Please download openapi.json from https://httpcan.org. Then create a static directory in the directory where the httpcan binary file is located, and place the downloaded openapi.json into that directory."
            })));
        }
    };

    // Parse the base OpenAPI JSON
    let mut openapi: Value = match serde_json::from_str(&base_openapi) {
        Ok(spec) => spec,
        Err(_) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to parse OpenAPI specification"
            })));
        }
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
    let body = serde_json::to_string(&catalog)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .content_type(r#"application/linkset+json; profile="https://www.rfc-editor.org/info/rfc9727""#)
        .body(body))
}
