use super::*;
use crate::handlers::utils::get_static_path;

pub async fn root_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    // Check the Accept header to determine response format
    let accept_header = req
        .headers()
        .get("accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    
    // If Accept header contains "html", serve the HTML page
    if accept_header.to_lowercase().contains("html") {
        // Try to serve the static index.html file
        let static_path = get_static_path();
        let index_path = static_path.join("index.html");
        
        match std::fs::read_to_string(&index_path) {
            Ok(html_content) => {
                Ok(HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(html_content))
            }
            Err(_) => {
                // Fallback to a helpful HTML response if index.html is not found
                let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
                let fallback_html = format!(r#"<!DOCTYPE html>
<html>
<head>
    <title>HTTPCan v{}</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 40px; line-height: 1.6; color: #333; }}
        .container {{ max-width: 800px; margin: 0 auto; }}
        h1 {{ color: #2c3e50; }}
        .version {{ color: #7f8c8d; font-size: 0.9em; }}
        .message {{ background: #f8f9fa; padding: 20px; border-radius: 8px; border-left: 4px solid #3498db; margin: 20px 0; }}
        .code {{ background: #f4f4f4; padding: 2px 6px; border-radius: 3px; font-family: 'Monaco', 'Consolas', monospace; }}
        a {{ color: #3498db; text-decoration: none; }}
        a:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>HTTPCan <span class="version">v{}</span></h1>
        <p>A simple, high‑performance HTTP request & response service built with Rust and Actix Web. Fully compatible with [httpbin.org](https://httpbin.org), with modern streaming and AI‑friendly enhancements.</p>
        
        <div class="message">
            <h3>Setup Required</h3>
            <p><strong>index.html not found.</strong> To get the full web interface:</p>
            <ol>
                <li>Download <span class="code">index.html</span> from <a href="https://httpcan.org" target="_blank">https://httpcan.org</a></li>
                <li>Create a <span class="code">static</span> directory in the directory where the httpcan binary file is located</li>
                <li>Place the downloaded <span class="code">index.html</span> into that directory</li>
            </ol>
        </div>
        
        <h3>API Documentation</h3>
        <p>Visit <a href="/openapi.json">/openapi.json</a> for API documentation.</p>
        
        <h3>Quick Test</h3>
        <p>Try these endpoints:</p>
        <ul>
            <li><a href="/get">/get</a> - Test GET requests</li>
            <li><a href="/post">/post</a> - Test POST requests</li>
            <li><a href="/headers">/headers</a> - View request headers</li>
            <li><a href="/ip">/ip</a> - Get your IP address</li>
            <li><a href="/sse">/sse</a> - Server-Sent Events stream</li>
        </ul>
    </div>
</body>
</html>"#, version, version);
                Ok(HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(fallback_html))
            }
        }
    } else {
        // If Accept header doesn't contain "html", return OpenAPI specification
        // Use the same logic as /openapi.json endpoint
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
        let mut openapi: serde_json::Value = match serde_json::from_str(&base_openapi) {
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
}

