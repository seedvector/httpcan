use actix_web::{
    web, App, HttpServer,
    middleware::Logger,
    HttpResponse, Result,
};
use actix_files as fs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use indexmap::IndexMap;
use clap::Parser;

mod handlers;
use handlers::*;

/// HTTPCan - HTTP testing service similar to httpbin.org
#[derive(Parser)]
#[command(name = "httpcan")]
#[command(about = "A simple HTTP request & response service", long_about = None)]
#[command(version)]
struct Args {
    /// Port number to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,
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

// Note: index.html is now served by actix-files at root path

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Parse command line arguments
    let args = Args::parse();
    let port = args.port;

    println!("Starting HTTPCan server on http://0.0.0.0:{}", port);

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            // Static file service for all static files including index.html at root
            .service(fs::Files::new("/static", "./static").show_files_listing())
            .service(fs::Files::new("/", "./static").index_file("index.html"))
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
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
