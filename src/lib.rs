//! HTTPCan - HTTP Request & Response Service Library
//!
//! This library provides a programmatic way to start an HTTPCan server,
//! which is compatible with httpbin.org and provides various HTTP testing endpoints.

use actix_cors::Cors;
use actix_files as fs;
use actix_web::{
    guard, http::Method, web, App, FromRequest, Handler, HttpServer, Responder, Route,
};
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

/// Route factory matching GET **and** HEAD.
///
/// Drop-in parity with httpbin: Flask auto-serves HEAD on every GET route,
/// Go 1.22 `ServeMux` "GET" patterns also match HEAD, and RFC 9110 §9.3.2
/// defines HEAD as "identical to GET except" the body. Actix performs no
/// such auto-mapping, so all GET endpoints register through this helper.
/// The HTTP codec strips the body of HEAD responses while preserving the
/// Content-Length of the would-be GET body, so handlers stay untouched.
fn get_or_head<F, Args>(handler: F) -> Route
where
    F: Handler<Args>,
    F::Output: Responder + 'static,
    Args: FromRequest + 'static,
{
    web::route()
        .guard(guard::Any(guard::Get()).or(guard::Head()))
        .to(handler)
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
        .service(web::resource("/openapi.json").route(get_or_head(openapi_handler)))
        // RFC 9727 API catalog for automated API discovery
        .service(web::resource("/.well-known/api-catalog").route(get_or_head(api_catalog_handler)));

    // Only add static file services if the static directory exists
    if static_path.exists() {
        app = app.service(fs::Files::new("/static", &static_path));
    }

    app = app
        // Body reflection — returns the request body verbatim with mirrored
        // headers. /body is the facet-named primary (/headers, /body, /method,
        // ...); /echo is kept as a compatibility alias for pre-0.7 clients.
        .service(
            web::resource("/body")
                .route(get_or_head(echo_handler_get))
                .route(web::post().to(echo_handler))
                .route(web::put().to(echo_handler))
                .route(web::patch().to(echo_handler))
                .route(web::delete().to(echo_handler))
                .route(web::method(query_method()).to(echo_handler)),
        )
        // Compatibility alias of /body
        .service(
            web::resource("/echo")
                .route(get_or_head(echo_handler_get))
                .route(web::post().to(echo_handler))
                .route(web::put().to(echo_handler))
                .route(web::patch().to(echo_handler))
                .route(web::delete().to(echo_handler))
                .route(web::method(query_method()).to(echo_handler)),
        )
        // HTTP Methods
        .service(web::resource("/get").route(get_or_head(get_handler)))
        .service(web::resource("/post").route(web::post().to(post_handler)))
        .service(web::resource("/put").route(web::put().to(put_handler)))
        .service(web::resource("/patch").route(web::patch().to(patch_handler)))
        .service(web::resource("/delete").route(web::delete().to(delete_handler)))
        // Method echo - accepts ANY HTTP method name (httpbin #522)
        .service(web::resource("/method").route(web::to(method_handler)))
        // HEAD-only endpoint echoing headers as X-Echo-* (httpbin #630)
        .service(web::resource("/head").route(web::head().to(head_handler)))
        // Dedicated echo endpoints completing the standard-method family
        // (OPTIONS: RFC 9110 §9.3.7, TRACE: §9.8, QUERY: RFC 9430).
        .service(
            web::resource("/options")
                .route(web::method(actix_web::http::Method::OPTIONS).to(options_handler)),
        )
        .service(web::resource("/trace").route(web::trace().to(trace_handler)))
        .service(web::resource("/query").route(web::method(query_method()).to(query_handler)))
        // Anything endpoints - supporting multiple methods
        .service(
            web::resource("/anything")
                .route(get_or_head(anything_handler_get))
                .route(web::post().to(anything_handler))
                .route(web::put().to(anything_handler))
                .route(web::patch().to(anything_handler))
                .route(web::delete().to(anything_handler))
                .route(web::trace().to(anything_handler_get))
                .route(web::method(actix_web::http::Method::OPTIONS).to(anything_handler_get))
                .route(web::method(query_method()).to(anything_handler)),
        )
        // Support for any path after /anything (single or multi-segment)
        .service(
            web::resource("/anything/{path:.*}")
                .route(get_or_head(anything_with_param_handler_get))
                .route(web::post().to(anything_with_param_handler))
                .route(web::put().to(anything_with_param_handler))
                .route(web::patch().to(anything_with_param_handler))
                .route(web::delete().to(anything_with_param_handler))
                .route(web::trace().to(anything_with_param_handler_get))
                .route(
                    web::method(actix_web::http::Method::OPTIONS)
                        .to(anything_with_param_handler_get),
                )
                .route(web::method(query_method()).to(anything_with_param_handler)),
        )
        // Auth endpoints
        .service(
            web::resource("/basic-auth/{user}/{passwd}")
                .route(get_or_head(basic_auth_handler))
                .route(web::post().to(basic_auth_handler)),
        )
        .service(
            web::resource("/basic-auth/{user}")
                .route(get_or_head(basic_auth_user_only_handler))
                .route(web::post().to(basic_auth_user_only_handler)),
        )
        .service(
            web::resource("/hidden-basic-auth/{user}/{passwd}")
                .route(get_or_head(hidden_basic_auth_handler))
                .route(web::post().to(hidden_basic_auth_handler)),
        )
        .service(
            web::resource("/hidden-basic-auth/{user}")
                .route(get_or_head(hidden_basic_auth_user_only_handler))
                .route(web::post().to(hidden_basic_auth_user_only_handler)),
        )
        .service(web::resource("/bearer").route(get_or_head(bearer_auth_handler)))
        .service(web::resource("/jwt-bearer").route(get_or_head(jwt_bearer_handler)))
        // Digest auth endpoints - support both GET and POST for auth-int with body
        .service(
            web::resource("/digest-auth/{qop}/{user}/{passwd}")
                .route(get_or_head(digest_auth_handler))
                .route(web::post().to(digest_auth_handler)),
        )
        .service(
            web::resource("/digest-auth/{qop}/{user}/{passwd}/{algorithm}")
                .route(get_or_head(digest_auth_with_algorithm_handler))
                .route(web::post().to(digest_auth_with_algorithm_handler)),
        )
        .service(
            web::resource("/digest-auth/{qop}/{user}/{passwd}/{algorithm}/{stale_after}")
                .route(get_or_head(digest_auth_full_handler))
                .route(web::post().to(digest_auth_full_handler)),
        )
        // Response formats
        .service(web::resource("/json").route(get_or_head(json_handler)))
        .service(web::resource("/xml").route(get_or_head(xml_handler)))
        .service(web::resource("/html").route(get_or_head(html_handler)))
        .service(web::resource("/robots.txt").route(get_or_head(robots_txt_handler)))
        .service(web::resource("/sitemap.xml").route(get_or_head(sitemap_handler)))
        .service(web::resource("/deny").route(get_or_head(deny_handler)))
        .service(web::resource("/encoding/utf8").route(get_or_head(utf8_handler)))
        .service(web::resource("/encoding/iso-8859-1").route(get_or_head(iso_8859_1_handler)))
        .service(
            web::resource("/gzip")
                .route(get_or_head(gzip_handler))
                .route(web::post().to(compress_post_handler)),
        )
        .service(
            web::resource("/deflate")
                .route(get_or_head(deflate_handler))
                .route(web::post().to(compress_post_handler)),
        )
        .service(
            web::resource("/brotli")
                .route(get_or_head(brotli_handler))
                .route(web::post().to(compress_post_handler)),
        )
        .service(
            web::resource("/zstd")
                .route(get_or_head(zstd_handler))
                .route(web::post().to(compress_post_handler)),
        )
        // Dynamic data
        .service(web::resource("/uuid").route(get_or_head(uuid_handler)))
        .service(web::resource("/base64/{value}").route(get_or_head(base64_handler)))
        .service(web::resource("/base64").route(web::post().to(base64_post_handler)))
        .service(web::resource("/bytes/{n}").route(get_or_head(bytes_handler)))
        .service(web::resource("/stream-bytes/{n}").route(get_or_head(stream_bytes_handler)))
        .service(web::resource("/stream/{n}").route(get_or_head(stream_handler)))
        .service(web::resource("/range/{numbytes}").route(get_or_head(range_handler)))
        .service(web::resource("/links/{n}/{offset}").route(get_or_head(links_handler)))
        .service(web::resource("/links/{n}").route(get_or_head(links_redirect_handler)))
        .service(web::resource("/drip").route(get_or_head(drip_handler)))
        // Delay endpoint - supporting multiple methods
        .service(
            web::resource("/delay/{delay}")
                .route(get_or_head(delay_handler_get))
                .route(web::post().to(delay_handler))
                .route(web::put().to(delay_handler))
                .route(web::patch().to(delay_handler))
                .route(web::delete().to(delay_handler))
                .route(web::trace().to(delay_handler_get)),
        )
        // Status codes - supporting multiple methods
        .service(
            web::resource("/status/{codes:.*}")
                .route(get_or_head(status_handler_get))
                .route(web::post().to(status_handler))
                .route(web::put().to(status_handler))
                .route(web::patch().to(status_handler))
                .route(web::delete().to(status_handler))
                .route(web::trace().to(status_handler_get))
                .route(web::method(actix_web::http::Method::OPTIONS).to(status_options_handler)),
        )
        // Redirects
        .service(web::resource("/redirect/{n}").route(get_or_head(redirect_handler)))
        .service(
            web::resource("/relative-redirect/{n}").route(get_or_head(relative_redirect_handler)),
        )
        .service(
            web::resource("/absolute-redirect/{n}").route(get_or_head(absolute_redirect_handler)),
        )
        .service(
            web::resource("/redirect-to")
                .route(get_or_head(redirect_to_handler_get))
                .route(web::post().to(redirect_to_handler))
                .route(web::put().to(redirect_to_handler))
                .route(web::patch().to(redirect_to_handler))
                .route(web::delete().to(redirect_to_handler))
                .route(web::trace().to(redirect_to_handler_get)),
        )
        // Request inspection
        .service(web::resource("/headers").route(get_or_head(headers_handler)))
        .service(web::resource("/ip").route(get_or_head(ip_handler)))
        .service(web::resource("/user-agent").route(get_or_head(user_agent_handler)))
        // Response inspection
        .service(web::resource("/cache").route(get_or_head(cache_handler)))
        .service(web::resource("/cache/{value}").route(get_or_head(cache_control_handler)))
        .service(web::resource("/etag/{etag:.*}").route(get_or_head(etag_handler)))
        .service(
            web::resource("/response-headers")
                .route(get_or_head(response_headers_get_handler))
                .route(web::post().to(response_headers_post_handler)),
        )
        // Cookies
        .service(web::resource("/cookies").route(get_or_head(cookies_handler)))
        .service(web::resource("/cookies/set").route(get_or_head(cookies_set_handler)))
        .service(
            web::resource("/cookies/set/{name}/{value}")
                .route(get_or_head(cookies_set_named_handler)),
        )
        .service(web::resource("/cookies/delete").route(get_or_head(cookies_delete_handler)))
        // Observability & instance identification (httpbin #544/#565)
        .service(web::resource("/healthz").route(get_or_head(healthz_handler)))
        .service(web::resource("/tags").route(get_or_head(tags_handler)))
        .service(web::resource("/tags/{name}").route(get_or_head(tag_value_handler)))
        // Images
        .service(web::resource("/image").route(get_or_head(image_handler)))
        .service(web::resource("/image/png").route(get_or_head(image_png_handler)))
        .service(web::resource("/image/jpeg").route(get_or_head(image_jpeg_handler)))
        .service(web::resource("/image/webp").route(get_or_head(image_webp_handler)))
        .service(web::resource("/image/svg").route(get_or_head(image_svg_handler)))
        // Server-Sent Events (SSE)
        .service(web::resource("/sse").route(get_or_head(sse_handler)))
        .service(web::resource("/sse/{count}").route(get_or_head(sse_path_handler)))
        .service(
            web::resource("/sse/{count}/{delay}").route(get_or_head(sse_path_with_delay_handler)),
        )
        // NDJSON streaming endpoints
        .service(web::resource("/ndjson").route(get_or_head(ndjson_handler)))
        .service(web::resource("/ndjson/{count}").route(get_or_head(ndjson_path_handler)))
        .service(
            web::resource("/ndjson/{count}/{delay}")
                .route(get_or_head(ndjson_path_with_delay_handler)),
        )
        // Root endpoint - always renders the static homepage (see src/handlers/root.rs)
        .service(web::resource("/").route(get_or_head(root_handler)))
        // Favicon - embedded in the binary, overridable via static/favicon.png
        .service(web::resource("/favicon.png").route(get_or_head(favicon_handler)));

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
