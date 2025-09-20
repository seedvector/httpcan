use actix_web::{
    web, App, HttpServer,
    middleware::Logger,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

mod handlers;
use handlers::*;

#[derive(Serialize, Deserialize)]
struct RequestInfo {
    args: HashMap<String, String>,
    data: String,
    files: HashMap<String, String>,
    form: HashMap<String, String>,
    headers: HashMap<String, String>,
    json: Option<Value>,
    method: String,
    origin: String,
    url: String,
    user_agent: Option<String>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("Starting HTTPCan server on http://0.0.0.0:8080");

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            // HTTP Methods
            .route("/get", web::get().to(get_handler))
            .route("/post", web::post().to(post_handler))
            .route("/put", web::put().to(put_handler))
            .route("/patch", web::patch().to(patch_handler))
            .route("/delete", web::delete().to(delete_handler))
            
            // Anything endpoints - supporting multiple methods
            .route("/anything", web::route()
                .method(actix_web::http::Method::GET)
                .method(actix_web::http::Method::POST)
                .method(actix_web::http::Method::PUT)
                .method(actix_web::http::Method::PATCH)
                .method(actix_web::http::Method::DELETE)
                .method(actix_web::http::Method::TRACE)
                .to(anything_handler))
            .route("/anything/{anything}", web::route()
                .method(actix_web::http::Method::GET)
                .method(actix_web::http::Method::POST)
                .method(actix_web::http::Method::PUT)
                .method(actix_web::http::Method::PATCH)
                .method(actix_web::http::Method::DELETE)
                .method(actix_web::http::Method::TRACE)
                .to(anything_with_param_handler))
            
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
            .route("/delay/{delay}", web::route()
                .method(actix_web::http::Method::GET)
                .method(actix_web::http::Method::POST)
                .method(actix_web::http::Method::PUT)
                .method(actix_web::http::Method::PATCH)
                .method(actix_web::http::Method::DELETE)
                .method(actix_web::http::Method::TRACE)
                .to(delay_handler))
            
            // Status codes - supporting multiple methods
            .route("/status/{codes}", web::route()
                .method(actix_web::http::Method::GET)
                .method(actix_web::http::Method::POST)
                .method(actix_web::http::Method::PUT)
                .method(actix_web::http::Method::PATCH)
                .method(actix_web::http::Method::DELETE)
                .method(actix_web::http::Method::TRACE)
                .to(status_handler))
            
            // Redirects
            .route("/redirect/{n}", web::get().to(redirect_handler))
            .route("/relative-redirect/{n}", web::get().to(relative_redirect_handler))
            .route("/absolute-redirect/{n}", web::get().to(absolute_redirect_handler))
            .route("/redirect-to", web::route()
                .method(actix_web::http::Method::GET)
                .method(actix_web::http::Method::POST)
                .method(actix_web::http::Method::PUT)
                .method(actix_web::http::Method::PATCH)
                .method(actix_web::http::Method::DELETE)
                .method(actix_web::http::Method::TRACE)
                .to(redirect_to_handler))
            
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
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
