//! HTTPCan - HTTP Request & Response Service Library
//! 
//! This library provides a programmatic way to start an HTTPCan server,
//! which is compatible with httpbin.org and provides various HTTP testing endpoints.

use actix_web::{web, App, HttpServer};
use actix_files as fs;
use actix_cors::Cors;
use std::path::PathBuf;

pub mod config;
pub mod handlers;
pub mod middleware;

pub use config::AppConfig;
use handlers::*;
use middleware::RequestLogger;

/// Configuration for the HTTPCan server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port to bind the server to
    pub port: u16,
    /// Host address to bind to (default: "0.0.0.0")
    pub host: String,
    /// Whether to add current server to OpenAPI specification
    pub add_current_server: bool,
    /// Headers to exclude from responses
    pub exclude_headers: Vec<String>,
    /// Custom static files directory
    pub static_dir: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "0.0.0.0".to_string(),
            add_current_server: true,
            exclude_headers: Vec::new(),
            static_dir: None,
        }
    }
}

impl ServerConfig {
    /// Create a new server configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the port for the server
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the host address for the server
    pub fn host<S: Into<String>>(mut self, host: S) -> Self {
        self.host = host.into();
        self
    }

    /// Enable or disable adding current server to OpenAPI specification
    pub fn add_current_server(mut self, add: bool) -> Self {
        self.add_current_server = add;
        self
    }

    /// Set headers to exclude from responses
    pub fn exclude_headers(mut self, headers: Vec<String>) -> Self {
        self.exclude_headers = headers;
        self
    }

    /// Add a header to exclude from responses
    pub fn exclude_header<S: Into<String>>(mut self, header: S) -> Self {
        self.exclude_headers.push(header.into());
        self
    }

    /// Set custom static files directory
    pub fn static_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.static_dir = Some(dir.into());
        self
    }
}

/// HTTPCan server builder and runner
pub struct HttpCanServer {
    config: ServerConfig,
}

impl HttpCanServer {
    /// Create a new HTTPCan server with default configuration
    pub fn new() -> Self {
        Self {
            config: ServerConfig::default(),
        }
    }

    /// Create a new HTTPCan server with custom configuration
    pub fn with_config(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Set the port for the server
    pub fn port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    /// Set the host address for the server
    pub fn host<S: Into<String>>(mut self, host: S) -> Self {
        self.config.host = host.into();
        self
    }

    /// Enable or disable adding current server to OpenAPI specification
    pub fn add_current_server(mut self, add: bool) -> Self {
        self.config.add_current_server = add;
        self
    }

    /// Set headers to exclude from responses
    pub fn exclude_headers(mut self, headers: Vec<String>) -> Self {
        self.config.exclude_headers = headers;
        self
    }

    /// Add a header to exclude from responses
    pub fn exclude_header<S: Into<String>>(mut self, header: S) -> Self {
        self.config.exclude_headers.push(header.into());
        self
    }

    /// Set custom static files directory
    pub fn static_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.config.static_dir = Some(dir.into());
        self
    }

    /// Start the HTTPCan server
    pub async fn run(self) -> std::io::Result<()> {
        let bind_address = format!("{}:{}", self.config.host, self.config.port);
        
        println!("Starting HTTPCan server on http://{}", bind_address);
        if self.config.add_current_server {
            println!("OpenAPI will include current server in servers list");
        } else {
            println!("OpenAPI will use static servers list only");
        }

        let config = self.config.clone();
        
        HttpServer::new(move || {
            create_app(config.clone())
        })
        .bind(&bind_address)?
        .run()
        .await
    }

    /// Get the server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}

impl Default for HttpCanServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Create an Actix Web application with all HTTPCan routes
fn create_app(server_config: ServerConfig) -> App<impl actix_web::dev::ServiceFactory<actix_web::dev::ServiceRequest, Config = (), Response = actix_web::dev::ServiceResponse<actix_web::body::EitherBody<actix_web::body::BoxBody>>, Error = actix_web::Error, InitError = ()>> {
    let static_path = server_config.static_dir.unwrap_or_else(get_static_path);
    let app_config = AppConfig {
        add_current_server: server_config.add_current_server,
        exclude_headers: server_config.exclude_headers,
    };
    
    let mut app = App::new()
        .app_data(web::Data::new(app_config))
        .wrap(
            Cors::default()
                .allowed_origin_fn(|_origin, _req_head| {
                    // Dynamically set Origin to fully mimic httpbin behavior
                    // httpbin: response.headers["Access-Control-Allow-Origin"] = request.headers.get("Origin", "*")
                    true // Allow all origins, actix-cors will automatically echo Origin header or set to "*"
                })
                .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"])
                .allow_any_header()
                .supports_credentials() // Equivalent to Access-Control-Allow-Credentials: true
                .max_age(3600) // Equivalent to Access-Control-Max-Age: 3600
        )
        .wrap(RequestLogger)
        // Dynamic OpenAPI specification endpoint
        .route("/openapi.json", web::get().to(openapi_handler));
    
    // Only add static file services if the static directory exists
    if static_path.exists() {
        app = app.service(fs::Files::new("/static", &static_path).show_files_listing());
    }
    
    app = app
        // Echo endpoint - mirrors request body and headers
        .route("/echo", web::get().to(echo_handler_get))
        .route("/echo", web::post().to(echo_handler))
        .route("/echo", web::put().to(echo_handler))
        .route("/echo", web::patch().to(echo_handler))
        .route("/echo", web::delete().to(echo_handler))
        
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
        .route("/anything", web::trace().to(anything_handler_get))
        // Support for any path after /anything (single or multi-segment)
        .route("/anything/{path:.*}", web::get().to(anything_with_param_handler_get))
        .route("/anything/{path:.*}", web::post().to(anything_with_param_handler))
        .route("/anything/{path:.*}", web::put().to(anything_with_param_handler))
        .route("/anything/{path:.*}", web::patch().to(anything_with_param_handler))
        .route("/anything/{path:.*}", web::delete().to(anything_with_param_handler))
        .route("/anything/{path:.*}", web::trace().to(anything_with_param_handler_get))
        
        // Auth endpoints
        .route("/basic-auth/{user}/{passwd}", web::get().to(basic_auth_handler))
        .route("/basic-auth/{user}", web::get().to(basic_auth_user_only_handler))
        .route("/hidden-basic-auth/{user}/{passwd}", web::get().to(hidden_basic_auth_handler))
        .route("/hidden-basic-auth/{user}", web::get().to(hidden_basic_auth_user_only_handler))
        .route("/bearer", web::get().to(bearer_auth_handler))
        .route("/jwt-bearer", web::get().to(jwt_bearer_handler))
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
        .route("/links/{n}", web::get().to(links_redirect_handler))
        .route("/drip", web::get().to(drip_handler))
        
        // Delay endpoint - supporting multiple methods
        .route("/delay/{delay}", web::get().to(delay_handler_get))
        .route("/delay/{delay}", web::post().to(delay_handler))
        .route("/delay/{delay}", web::put().to(delay_handler))
        .route("/delay/{delay}", web::patch().to(delay_handler))
        .route("/delay/{delay}", web::delete().to(delay_handler))
        .route("/delay/{delay}", web::trace().to(delay_handler_get))
        
        // Status codes - supporting multiple methods
        .route("/status/{codes}", web::get().to(status_handler_get))
        .route("/status/{codes}", web::post().to(status_handler))
        .route("/status/{codes}", web::put().to(status_handler))
        .route("/status/{codes}", web::patch().to(status_handler))
        .route("/status/{codes}", web::delete().to(status_handler))
        .route("/status/{codes}", web::trace().to(status_handler_get))
        .route("/status/{codes}", web::method(actix_web::http::Method::OPTIONS).to(status_options_handler))
        
        // Redirects
        .route("/redirect/{n}", web::get().to(redirect_handler))
        .route("/relative-redirect/{n}", web::get().to(relative_redirect_handler))
        .route("/absolute-redirect/{n}", web::get().to(absolute_redirect_handler))
        .route("/redirect-to", web::get().to(redirect_to_handler_get))
        .route("/redirect-to", web::post().to(redirect_to_handler))
        .route("/redirect-to", web::put().to(redirect_to_handler))
        .route("/redirect-to", web::patch().to(redirect_to_handler))
        .route("/redirect-to", web::delete().to(redirect_to_handler))
        .route("/redirect-to", web::trace().to(redirect_to_handler_get))
        
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
        
        // Server-Sent Events (SSE)
        .route("/sse", web::get().to(sse_handler))
        .route("/sse/{count}", web::get().to(sse_path_handler))
        .route("/sse/{count}/{delay}", web::get().to(sse_path_with_delay_handler))
        
        // NDJSON streaming endpoints
        .route("/ndjson", web::get().to(ndjson_handler))
        .route("/ndjson/{count}", web::get().to(ndjson_path_handler))
        .route("/ndjson/{count}/{delay}", web::get().to(ndjson_path_with_delay_handler))
        
        // Root endpoint - returns HTML or API info based on Accept header
        .route("/", web::get().to(root_handler));
    
    // Only add root static file service if the static directory exists
    // This is added after all routes to serve as fallback for static resources
    if static_path.exists() {
        app = app.service(fs::Files::new("/", &static_path).index_file("index.html"));
    }
    
    app
}

/// Get the default static files path
fn get_static_path() -> PathBuf {
    handlers::utils::get_static_path()
}
