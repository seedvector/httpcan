use actix_web::{
    web, App, HttpServer,
    middleware::Logger,
};
use actix_files as fs;
use actix_cors::Cors;
use clap::Parser;

mod config;
mod handlers;

use config::{AppConfig, Args};
use handlers::*;


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
        
        let mut app = App::new()
            .app_data(web::Data::new(config))
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
            .wrap(Logger::default())
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
            .route("/redirect-to", web::delete().to(redirect_to_handler_get))
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
            .route("/ndjson/{count}/{delay}", web::get().to(ndjson_path_with_delay_handler));
        
        // Only add root static file service if the static directory exists
        if static_path.exists() {
            app = app.service(fs::Files::new("/", &static_path).index_file("index.html"));
        }
        
        app
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
