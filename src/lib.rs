//! HTTPCan - HTTP Request & Response Service Library
//!
//! This library provides a programmatic way to start an HTTPCan server,
//! which is compatible with httpbin.org and provides various HTTP testing endpoints.

use actix_cors::Cors;
use actix_files as fs;
use actix_web::{http::Method, web, App, HttpServer};
use std::path::PathBuf;

pub mod config;
pub mod handlers;
pub mod middleware;

/// Runtime application configuration shared with request handlers via
/// `web::Data`. Constructed from [`ServerConfig`] in `create_app`.
#[derive(Clone)]
pub struct AppConfig {
    pub add_current_server: bool,
    pub exclude_headers: Vec<String>,
    /// Maximum bytes served by `/bytes` and `/stream-bytes`. Requests
    /// exceeding this return a 404 instead of silently truncating (httpbin #594).
    pub max_bytes: usize,
    /// Canonical scheme for SEO-facing URLs (see
    /// [`config::SchemeOverride`]).
    pub canonical_scheme: config::SchemeOverride,
    /// Resolved static assets directory (`ServerConfig::static_dir`, else the
    /// `static` dir next to the binary, else `./static`). User override files
    /// placed here replace built-in defaults (see `handlers::utils`).
    pub static_path: PathBuf,
}
use handlers::*;
use middleware::RequestLogger;

/// The HTTP QUERY method (RFC 9430): a safe, idempotent, cacheable method that
/// carries a request body — semantically a GET with a body.
fn query_method() -> Method {
    Method::from_bytes(b"QUERY").expect("\"QUERY\" is a valid HTTP method token")
}

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
    /// Maximum bytes for `/bytes` and `/stream-bytes` (httpbin #594)
    pub max_bytes: usize,
    /// Canonical scheme for SEO-facing URLs (see
    /// [`config::SchemeOverride`]).
    pub canonical_scheme: config::SchemeOverride,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "0.0.0.0".to_string(),
            add_current_server: true,
            exclude_headers: Vec::new(),
            static_dir: None,
            max_bytes: config::DEFAULT_MAX_BYTES,
            canonical_scheme: config::SchemeOverride::Auto,
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

    /// Set the maximum bytes for `/bytes` and `/stream-bytes` (httpbin #594)
    pub fn max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Set the canonical scheme override for self-referential SEO URLs (see
    /// [`config::SchemeOverride`]).
    pub fn canonical_scheme(mut self, canonical_scheme: config::SchemeOverride) -> Self {
        self.canonical_scheme = canonical_scheme;
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

    /// Set the maximum bytes for `/bytes` and `/stream-bytes` (httpbin #594)
    pub fn max_bytes(mut self, max_bytes: usize) -> Self {
        self.config.max_bytes = max_bytes;
        self
    }

    /// Set the canonical scheme override for self-referential SEO URLs (see
    /// [`config::SchemeOverride`]).
    pub fn canonical_scheme(mut self, canonical_scheme: config::SchemeOverride) -> Self {
        self.config.canonical_scheme = canonical_scheme;
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

        // Visibility: which static assets the operator is overriding.
        let static_path = self
            .config
            .static_dir
            .clone()
            .unwrap_or_else(handlers::utils::get_static_path);
        println!("Static assets dir: {}", static_path.display());
        for name in [
            "openapi.json",
            "favicon.png",
            "index.html",
            "robots.txt",
            "sitemap.xml",
        ] {
            if static_path.join(name).exists() {
                println!("Override active: static/{name} replaces built-in default");
            }
        }

        let config = self.config.clone();

        HttpServer::new(move || create_app(config.clone()))
            .client_request_timeout(std::time::Duration::from_secs(120))
            .client_disconnect_timeout(std::time::Duration::from_secs(10))
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
fn create_app(
    server_config: ServerConfig,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<
            actix_web::body::EitherBody<actix_web::body::BoxBody>,
        >,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let static_path = server_config.static_dir.unwrap_or_else(get_static_path);
    let app_config = AppConfig {
        add_current_server: server_config.add_current_server,
        exclude_headers: server_config.exclude_headers,
        max_bytes: server_config.max_bytes,
        canonical_scheme: server_config.canonical_scheme,
        static_path: static_path.clone(),
    };

    let mut app = App::new()
        .app_data(web::Data::new(app_config))
        // Increase payload size limit for large static files
        .app_data(web::PayloadConfig::new(10 * 1024 * 1024)) // 10MB limit
        .wrap(
            Cors::default()
                .allowed_origin_fn(|_origin, _req_head| {
                    // Dynamically set Origin to fully mimic httpbin behavior
                    // httpbin: response.headers["Access-Control-Allow-Origin"] = request.headers.get("Origin", "*")
                    true // Allow all origins, actix-cors will automatically echo Origin header or set to "*"
                })
                .allowed_methods(vec![
                    "GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "QUERY",
                ])
                .allow_any_header()
                .expose_headers(["WWW-Authenticate"])
                .supports_credentials() // Equivalent to Access-Control-Allow-Credentials: true
                .max_age(3600), // Equivalent to Access-Control-Max-Age: 3600
        )
        .wrap(RequestLogger)
        // Dynamic OpenAPI specification endpoint
        .route("/openapi.json", web::get().to(openapi_handler))
        // RFC 9727 API catalog for automated API discovery
        .route(
            "/.well-known/api-catalog",
            web::get().to(api_catalog_handler),
        );

    // Only add static file services if the static directory exists
    if static_path.exists() {
        app = app.service(fs::Files::new("/static", &static_path));
    }

    app = app
        // Echo endpoint - mirrors request body and headers
        .route("/echo", web::get().to(echo_handler_get))
        .route("/echo", web::post().to(echo_handler))
        .route("/echo", web::put().to(echo_handler))
        .route("/echo", web::patch().to(echo_handler))
        .route("/echo", web::delete().to(echo_handler))
        .route("/echo", web::method(query_method()).to(echo_handler))
        // HTTP Methods
        .route("/get", web::get().to(get_handler))
        .route("/post", web::post().to(post_handler))
        .route("/put", web::put().to(put_handler))
        .route("/patch", web::patch().to(patch_handler))
        .route("/delete", web::delete().to(delete_handler))
        // Method echo - accepts ANY HTTP method name (httpbin #522)
        .route("/method", web::to(method_handler))
        // HEAD-only endpoint echoing headers as X-Echo-* (httpbin #630)
        .route("/head", web::head().to(head_handler))
        // Anything endpoints - supporting multiple methods
        .route("/anything", web::get().to(anything_handler_get))
        .route("/anything", web::post().to(anything_handler))
        .route("/anything", web::put().to(anything_handler))
        .route("/anything", web::patch().to(anything_handler))
        .route("/anything", web::delete().to(anything_handler))
        .route("/anything", web::trace().to(anything_handler_get))
        .route(
            "/anything",
            web::method(query_method()).to(anything_handler),
        )
        // Support for any path after /anything (single or multi-segment)
        .route(
            "/anything/{path:.*}",
            web::get().to(anything_with_param_handler_get),
        )
        .route(
            "/anything/{path:.*}",
            web::post().to(anything_with_param_handler),
        )
        .route(
            "/anything/{path:.*}",
            web::put().to(anything_with_param_handler),
        )
        .route(
            "/anything/{path:.*}",
            web::patch().to(anything_with_param_handler),
        )
        .route(
            "/anything/{path:.*}",
            web::delete().to(anything_with_param_handler),
        )
        .route(
            "/anything/{path:.*}",
            web::trace().to(anything_with_param_handler_get),
        )
        .route(
            "/anything/{path:.*}",
            web::method(query_method()).to(anything_with_param_handler),
        )
        // Auth endpoints
        .route(
            "/basic-auth/{user}/{passwd}",
            web::get().to(basic_auth_handler),
        )
        .route(
            "/basic-auth/{user}",
            web::get().to(basic_auth_user_only_handler),
        )
        .route(
            "/hidden-basic-auth/{user}/{passwd}",
            web::get().to(hidden_basic_auth_handler),
        )
        .route(
            "/hidden-basic-auth/{user}",
            web::get().to(hidden_basic_auth_user_only_handler),
        )
        .route(
            "/basic-auth/{user}/{passwd}",
            web::post().to(basic_auth_handler),
        )
        .route(
            "/basic-auth/{user}",
            web::post().to(basic_auth_user_only_handler),
        )
        .route(
            "/hidden-basic-auth/{user}/{passwd}",
            web::post().to(hidden_basic_auth_handler),
        )
        .route(
            "/hidden-basic-auth/{user}",
            web::post().to(hidden_basic_auth_user_only_handler),
        )
        .route("/bearer", web::get().to(bearer_auth_handler))
        .route("/jwt-bearer", web::get().to(jwt_bearer_handler))
        // Digest auth endpoints - support both GET and POST for auth-int with body
        .route(
            "/digest-auth/{qop}/{user}/{passwd}",
            web::get().to(digest_auth_handler),
        )
        .route(
            "/digest-auth/{qop}/{user}/{passwd}",
            web::post().to(digest_auth_handler),
        )
        .route(
            "/digest-auth/{qop}/{user}/{passwd}/{algorithm}",
            web::get().to(digest_auth_with_algorithm_handler),
        )
        .route(
            "/digest-auth/{qop}/{user}/{passwd}/{algorithm}",
            web::post().to(digest_auth_with_algorithm_handler),
        )
        .route(
            "/digest-auth/{qop}/{user}/{passwd}/{algorithm}/{stale_after}",
            web::get().to(digest_auth_full_handler),
        )
        .route(
            "/digest-auth/{qop}/{user}/{passwd}/{algorithm}/{stale_after}",
            web::post().to(digest_auth_full_handler),
        )
        // Response formats
        .route("/json", web::get().to(json_handler))
        .route("/xml", web::get().to(xml_handler))
        .route("/html", web::get().to(html_handler))
        .route("/robots.txt", web::get().to(robots_txt_handler))
        .route("/sitemap.xml", web::get().to(sitemap_handler))
        .route("/deny", web::get().to(deny_handler))
        .route("/encoding/utf8", web::get().to(utf8_handler))
        .route("/encoding/iso-8859-1", web::get().to(iso_8859_1_handler))
        .route("/gzip", web::get().to(gzip_handler))
        .route("/deflate", web::get().to(deflate_handler))
        .route("/brotli", web::get().to(brotli_handler))
        .route("/zstd", web::get().to(zstd_handler))
        .route("/gzip", web::post().to(compress_post_handler))
        .route("/deflate", web::post().to(compress_post_handler))
        .route("/brotli", web::post().to(compress_post_handler))
        .route("/zstd", web::post().to(compress_post_handler))
        // Dynamic data
        .route("/uuid", web::get().to(uuid_handler))
        .route("/base64/{value}", web::get().to(base64_handler))
        .route("/base64", web::post().to(base64_post_handler))
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
        .route("/status/{codes:.*}", web::get().to(status_handler_get))
        .route("/status/{codes:.*}", web::post().to(status_handler))
        .route("/status/{codes:.*}", web::put().to(status_handler))
        .route("/status/{codes:.*}", web::patch().to(status_handler))
        .route("/status/{codes:.*}", web::delete().to(status_handler))
        .route("/status/{codes:.*}", web::trace().to(status_handler_get))
        .route(
            "/status/{codes:.*}",
            web::method(actix_web::http::Method::OPTIONS).to(status_options_handler),
        )
        // Redirects
        .route("/redirect/{n}", web::get().to(redirect_handler))
        .route(
            "/relative-redirect/{n}",
            web::get().to(relative_redirect_handler),
        )
        .route(
            "/absolute-redirect/{n}",
            web::get().to(absolute_redirect_handler),
        )
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
        .route("/etag/{etag:.*}", web::get().to(etag_handler))
        .route(
            "/response-headers",
            web::get().to(response_headers_get_handler),
        )
        .route(
            "/response-headers",
            web::post().to(response_headers_post_handler),
        )
        // Cookies
        .route("/cookies", web::get().to(cookies_handler))
        .route("/cookies/set", web::get().to(cookies_set_handler))
        .route(
            "/cookies/set/{name}/{value}",
            web::get().to(cookies_set_named_handler),
        )
        .route("/cookies/delete", web::get().to(cookies_delete_handler))
        // Observability & instance identification (httpbin #544/#565)
        .route("/healthz", web::get().to(healthz_handler))
        .route("/tags", web::get().to(tags_handler))
        .route("/tags/{name}", web::get().to(tag_value_handler))
        // Images
        .route("/image", web::get().to(image_handler))
        .route("/image/png", web::get().to(image_png_handler))
        .route("/image/jpeg", web::get().to(image_jpeg_handler))
        .route("/image/webp", web::get().to(image_webp_handler))
        .route("/image/svg", web::get().to(image_svg_handler))
        // Server-Sent Events (SSE)
        .route("/sse", web::get().to(sse_handler))
        .route("/sse/{count}", web::get().to(sse_path_handler))
        .route(
            "/sse/{count}/{delay}",
            web::get().to(sse_path_with_delay_handler),
        )
        // NDJSON streaming endpoints
        .route("/ndjson", web::get().to(ndjson_handler))
        .route("/ndjson/{count}", web::get().to(ndjson_path_handler))
        .route(
            "/ndjson/{count}/{delay}",
            web::get().to(ndjson_path_with_delay_handler),
        )
        // Root endpoint - always renders the static homepage (see src/handlers/root.rs)
        .route("/", web::get().to(root_handler))
        // Favicon - embedded in the binary, overridable via static/favicon.png
        .route("/favicon.png", web::get().to(favicon_handler));

    // Only add root static file service if the static directory exists
    // This is added after all routes so explicit routes (API endpoints,
    // /favicon.png, …) always win over same-named user files.
    if static_path.exists() {
        app = app.service(
            fs::Files::new("/", &static_path)
                .prefer_utf8(true)
                .use_last_modified(true)
                .use_etag(true),
        );
    }

    app
}

/// Get the default static files path
fn get_static_path() -> PathBuf {
    handlers::utils::get_static_path()
}

#[cfg(test)]
mod tests;
