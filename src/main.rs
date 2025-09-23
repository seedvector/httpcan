use actix_web::{
    web, App, HttpServer, HttpResponse, HttpRequest, Result,
    middleware::Logger,
};
use actix_files as fs;
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use indexmap::IndexMap;
use clap::Parser;
use std::env;
use std::path::PathBuf;

mod handlers;
use handlers::*;

// Application configuration
#[derive(Clone)]
struct AppConfig {
    add_current_server: bool,
    exclude_headers: Vec<String>,
}

/// HTTPCan - HTTP testing service similar to httpbin.org
#[derive(Parser)]
#[command(name = "httpcan")]
#[command(about = "A simple HTTP request & response service", long_about = None)]
#[command(version)]
struct Args {
    /// Port number to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,
    
    /// Do not add current server to OpenAPI specification servers list
    #[arg(long)]
    no_current_server: bool,
    
    /// Exclude specific headers from responses. Comma-separated list of header keys, supports wildcard suffix matching (e.g., "foo, x-bar-*")
    #[arg(long)]
    exclude_headers: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct RequestInfo {
    args: IndexMap<String, String>,
    data: String,
    files: IndexMap<String, String>,
    form: IndexMap<String, String>,
    headers: IndexMap<String, String>,
    json: Option<Value>,
    method: String,
    origin: String,
    url: String,
    user_agent: Option<String>,
}

// Simplified response structure for GET requests (httpbin.org compatible)
#[derive(Serialize, Deserialize)]
struct GetRequestInfo {
    args: IndexMap<String, String>,
    headers: IndexMap<String, String>,
    origin: String,
    url: String,
}

// Helper function to get static directory path relative to executable
fn get_static_path() -> PathBuf {
    let exe_path = env::current_exe().unwrap();
    let exe_dir = exe_path.parent().unwrap();
    let static_path = exe_dir.join("static");
    
    // Fallback to current directory if static directory doesn't exist next to executable
    if !static_path.exists() {
        let current_dir_static = PathBuf::from("./static");
        if current_dir_static.exists() {
            return current_dir_static;
        }
    }
    
    static_path
}

// Generate dynamic OpenAPI specification with current server information
async fn openapi_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let static_path = get_static_path();
    let openapi_path = static_path.join("openapi.json");
    
    // Read the base OpenAPI specification
    let base_openapi = match std::fs::read_to_string(&openapi_path) {
        Ok(content) => content,
        Err(_) => {
            return Ok(HttpResponse::NotFound().json(json!({
                "error": "OpenAPI specification not found"
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Parse command line arguments
    let args = Args::parse();
    let port = args.port;
    let add_current_server = !args.no_current_server;
    
    // Parse exclude headers
    let exclude_headers: Vec<String> = args.exclude_headers
        .map(|headers_str| {
            headers_str
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    println!("Starting HTTPCan server on http://0.0.0.0:{}", port);
    if add_current_server {
        println!("OpenAPI will include current server in servers list");
    } else {
        println!("OpenAPI will use static servers list only");
    }

    HttpServer::new(move || {
        let static_path = get_static_path();
        let config = AppConfig {
            add_current_server,
            exclude_headers: exclude_headers.clone(),
        };
        
        App::new()
            .app_data(web::Data::new(config))
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600)
            )
            .wrap(Logger::default())
            // Dynamic OpenAPI specification endpoint
            .route("/openapi.json", web::get().to(openapi_handler))
            // Static file service for explicit /static path
            .service(fs::Files::new("/static", &static_path).show_files_listing())
            // HTTP Methods
            .route("/get", web::get().to(get_handler))
            .route("/post", web::post().to(post_handler))
            .route("/put", web::put().to(put_handler))
            .route("/patch", web::patch().to(patch_handler))
            .route("/delete", web::delete().to(delete_handler))
            
            // Anything endpoints - supporting multiple methods
            .route("/anything", web::get().to(anything_handler_get))
            .route("/anything", web::post().to(anything_handler))
            .route("/anything", web::put().to(anything_handler))
            .route("/anything", web::patch().to(anything_handler))
            .route("/anything", web::delete().to(anything_handler))
            // Support for any path after /anything (single or multi-segment)
            .route("/anything/{path:.*}", web::get().to(anything_with_param_handler_get))
            .route("/anything/{path:.*}", web::post().to(anything_with_param_handler))
            .route("/anything/{path:.*}", web::put().to(anything_with_param_handler))
            .route("/anything/{path:.*}", web::patch().to(anything_with_param_handler))
            .route("/anything/{path:.*}", web::delete().to(anything_with_param_handler))
            
            // Auth endpoints
            .route("/basic-auth/{user}/{passwd}", web::get().to(basic_auth_handler))
            .route("/hidden-basic-auth/{user}/{passwd}", web::get().to(hidden_basic_auth_handler))
            .route("/bearer", web::get().to(bearer_auth_handler))
            .route("/digest-auth/{qop}/{user}/{passwd}", web::get().to(digest_auth_handler))
            .route("/digest-auth/{qop}/{user}/{passwd}/{algorithm}", web::get().to(digest_auth_with_algorithm_handler))
            .route("/digest-auth/{qop}/{user}/{passwd}/{algorithm}/{stale_after}", web::get().to(digest_auth_full_handler))
            
            // Response formats
            .route("/json", web::get().to(json_handler))
            .route("/xml", web::get().to(xml_handler))
            .route("/html", web::get().to(html_handler))
            .route("/robots.txt", web::get().to(robots_txt_handler))
            .route("/deny", web::get().to(deny_handler))
            .route("/encoding/utf8", web::get().to(utf8_handler))
            .route("/gzip", web::get().to(gzip_handler))
            .route("/deflate", web::get().to(deflate_handler))
            .route("/brotli", web::get().to(brotli_handler))
            
            // Dynamic data
            .route("/uuid", web::get().to(uuid_handler))
            .route("/base64/{value}", web::get().to(base64_handler))
            .route("/bytes/{n}", web::get().to(bytes_handler))
            .route("/stream-bytes/{n}", web::get().to(stream_bytes_handler))
            .route("/stream/{n}", web::get().to(stream_handler))
            .route("/range/{numbytes}", web::get().to(range_handler))
            .route("/links/{n}/{offset}", web::get().to(links_handler))
            .route("/drip", web::get().to(drip_handler))
            
            // Delay endpoint - supporting multiple methods
            .route("/delay/{delay}", web::get().to(delay_handler_get))
            .route("/delay/{delay}", web::post().to(delay_handler))
            .route("/delay/{delay}", web::put().to(delay_handler))
            .route("/delay/{delay}", web::patch().to(delay_handler))
            .route("/delay/{delay}", web::delete().to(delay_handler))
            
            // Status codes - supporting multiple methods
            .route("/status/{codes}", web::get().to(status_handler_get))
            .route("/status/{codes}", web::post().to(status_handler))
            .route("/status/{codes}", web::put().to(status_handler))
            .route("/status/{codes}", web::patch().to(status_handler))
            .route("/status/{codes}", web::delete().to(status_handler))
            
            // Redirects
            .route("/redirect/{n}", web::get().to(redirect_handler))
            .route("/relative-redirect/{n}", web::get().to(relative_redirect_handler))
            .route("/absolute-redirect/{n}", web::get().to(absolute_redirect_handler))
            .route("/redirect-to", web::get().to(redirect_to_handler_get))
            .route("/redirect-to", web::post().to(redirect_to_handler))
            .route("/redirect-to", web::put().to(redirect_to_handler_get))
            .route("/redirect-to", web::patch().to(redirect_to_handler_get))
            .route("/redirect-to", web::delete().to(redirect_to_handler_get))
            
            // Request inspection
            .route("/headers", web::get().to(headers_handler))
            .route("/ip", web::get().to(ip_handler))
            .route("/user-agent", web::get().to(user_agent_handler))
            
            // Response inspection
            .route("/cache", web::get().to(cache_handler))
            .route("/cache/{value}", web::get().to(cache_control_handler))
            .route("/etag/{etag}", web::get().to(etag_handler))
            .route("/response-headers", web::get().to(response_headers_get_handler))
            .route("/response-headers", web::post().to(response_headers_post_handler))
            
            // Cookies
            .route("/cookies", web::get().to(cookies_handler))
            .route("/cookies/set", web::get().to(cookies_set_handler))
            .route("/cookies/set/{name}/{value}", web::get().to(cookies_set_named_handler))
            .route("/cookies/delete", web::get().to(cookies_delete_handler))
            
            // Images
            .route("/image", web::get().to(image_handler))
            .route("/image/png", web::get().to(image_png_handler))
            .route("/image/jpeg", web::get().to(image_jpeg_handler))
            .route("/image/webp", web::get().to(image_webp_handler))
            .route("/image/svg", web::get().to(image_svg_handler))
            // Serve static files from root after all API routes (index.html for root)
            .service(fs::Files::new("/", &static_path).index_file("index.html"))
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
