use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde_json::{Value, json};
use crate::config::AppConfig;
use crate::handlers::utils::get_static_path;

// Generate dynamic OpenAPI specification with current server information
pub async fn openapi_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
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
                    "description": "A simple HTTP request & response service built with Rust and Actix Web, with httpbin compatibility."
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
