use super::*;
use actix_multipart::Multipart;
use actix_web::web::BytesMut;
use futures_util::{StreamExt, TryStreamExt};
use serde_json::Value;
use std::collections::HashMap;
use url::form_urlencoded;

// Helper function to create case-insensitive parameter map
fn to_case_insensitive_map(
    params: &web::Query<HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut case_insensitive = HashMap::new();
    for (key, value) in params.iter() {
        case_insensitive.insert(key.to_lowercase(), value.clone());
    }
    case_insensitive
}

#[derive(Deserialize)]
pub struct RedirectQuery {
    absolute: Option<String>,
}

// Helper function to parse query string into HashMap
fn parse_query_params(query_string: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    if query_string.is_empty() {
        return params;
    }

    for pair in query_string.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            // URL decode the key and value
            if let (Ok(decoded_key), Ok(decoded_value)) =
                (urlencoding::decode(key), urlencoding::decode(value))
            {
                params.insert(decoded_key.to_string(), decoded_value.to_string());
            }
        } else if !pair.is_empty() {
            // Handle keys without values
            if let Ok(decoded_key) = urlencoding::decode(pair) {
                params.insert(decoded_key.to_string(), String::new());
            }
        }
    }

    params
}

// Helper function to extract parameters from request body based on content type
async fn extract_body_params(
    req: &HttpRequest,
    mut payload: web::Payload,
) -> Result<HashMap<String, String>> {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let mut params = HashMap::new();

    if content_type
        .to_lowercase()
        .starts_with("application/x-www-form-urlencoded")
    {
        // Parse URL-encoded form data
        let mut body = BytesMut::new();
        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            body.extend_from_slice(&chunk);
        }

        let body_string = String::from_utf8_lossy(&body);
        for (key, value) in form_urlencoded::parse(body_string.as_bytes()) {
            params.insert(key.to_string(), value.to_string());
        }
    } else if content_type
        .to_lowercase()
        .starts_with("multipart/form-data")
    {
        // Parse multipart form data
        let multipart = Multipart::new(req.headers(), payload);
        params = extract_multipart_params(multipart).await?;
    } else if content_type.to_lowercase().starts_with("application/json") {
        // Parse JSON data
        let mut body = BytesMut::new();
        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            body.extend_from_slice(&chunk);
        }

        let body_string = String::from_utf8_lossy(&body);
        if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&body_string) {
            for (key, value) in obj {
                if let Value::String(string_value) = value {
                    params.insert(key, string_value);
                } else {
                    // Convert non-string JSON values to strings
                    params.insert(key, value.to_string().trim_matches('"').to_string());
                }
            }
        }
    }

    Ok(params)
}

// Helper function to extract parameters from multipart form data
async fn extract_multipart_params(mut multipart: Multipart) -> Result<HashMap<String, String>> {
    let mut params = HashMap::new();

    while let Some(mut field) = multipart.try_next().await? {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition.get_name().map(|s| s.to_string());

        if let Some(name) = field_name {
            let mut data = Vec::new();

            // Read field data
            while let Some(chunk) = field.try_next().await? {
                data.extend_from_slice(&chunk);
            }

            // Only handle text fields for redirect parameters
            if let Ok(value) = String::from_utf8(data) {
                params.insert(name, value);
            }
        }
    }

    Ok(params)
}

// Helper function to convert HashMap to case-insensitive HashMap
fn to_case_insensitive_hashmap(params: &HashMap<String, String>) -> HashMap<String, String> {
    let mut case_insensitive = HashMap::new();
    for (key, value) in params.iter() {
        case_insensitive.insert(key.to_lowercase(), value.clone());
    }
    case_insensitive
}

/// Check if the request appears to be from a browser (HTML-capable client)
fn is_browser_request(req: &HttpRequest) -> bool {
    req.headers()
        .get("accept")
        .and_then(|h| h.to_str().ok())
        .map(|accept| accept.to_lowercase().contains("text/html"))
        .unwrap_or(false)
}

/// Check if a URL uses a safe scheme for clickable links (http/https only)
fn is_safe_url_scheme(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// HTML-escape a string for safe display in HTML content
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Build an HTML interstitial page for external redirect-to requests.
///
/// Instead of returning a 302 that silently redirects browser users to an
/// arbitrary URL, return a 200 HTML page that shows the destination and
/// requires a click. Programmatic clients (curl, etc.) still get the 302.
fn build_redirect_interstitial(url: &str) -> String {
    let escaped_url = html_escape(url);
    let link_html = if is_safe_url_scheme(url) {
        format!(
            r#"<a href="{}" rel="noopener noreferrer" class="btn-primary">Continue to external site</a>"#,
            escaped_url
        )
    } else {
        r#"<span class="btn-disabled">Link disabled: unsupported URL scheme</span>"#.to_string()
    };

    let template = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>External Link - HTTPCan</title>
<style>
body { font-family: system-ui, -apple-system, sans-serif; max-width: 640px; margin: 80px auto; padding: 0 20px; color: #1a1a1a; }
h1 { font-size: 1.5em; }
.warning { color: #c41e3a; font-weight: 600; }
.url { word-break: break-all; background: #f5f5f5; padding: 12px 16px; border-radius: 8px; font-family: monospace; font-size: 0.9em; margin: 16px 0; }
.actions { margin-top: 24px; }
.actions a, .actions span { display: inline-block; padding: 10px 24px; border-radius: 8px; margin-right: 12px; font-weight: 500; text-decoration: none; }
.btn-primary { background: #c41e3a; color: #fff; }
.btn-secondary { background: #e8e8e8; color: #333; }
.btn-disabled { color: #999; }
</style>
</head>
<body>
<h1>External Link</h1>
<p class="warning">&#9888; You are about to leave HTTPCan and visit an external website.</p>
<p>Destination URL:</p>
<div class="url">__URL__</div>
<p>HTTPCan does not control the content of external sites and cannot guarantee their safety. Proceed only if you trust this destination.</p>
<div class="actions">
__LINK__
<a href="/" class="btn-secondary">Return to HTTPCan</a>
</div>
</body>
</html>"#;

    template
        .replace("__URL__", &escaped_url)
        .replace("__LINK__", &link_html)
}

pub async fn redirect_handler(
    req: HttpRequest,
    path: web::Path<usize>,
    query: web::Query<RedirectQuery>,
) -> Result<HttpResponse> {
    let n = path.into_inner();

    if n == 0 {
        return Err(actix_web::error::ErrorBadRequest(
            "n must be greater than 0",
        ));
    }

    let absolute = query
        .absolute
        .as_ref()
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false);

    if n == 1 {
        // Final redirect to /get
        if absolute {
            let host = req.connection_info().host().to_string();
            let scheme = if req.connection_info().scheme() == "https" {
                "https"
            } else {
                "http"
            };
            Ok(HttpResponse::Found()
                .append_header(("Location", format!("{}://{}/get", scheme, host)))
                .body(""))
        } else {
            Ok(HttpResponse::Found()
                .append_header(("Location", "/get"))
                .body(""))
        }
    } else {
        // Redirect to the next step
        if absolute {
            let host = req.connection_info().host().to_string();
            let scheme = if req.connection_info().scheme() == "https" {
                "https"
            } else {
                "http"
            };
            Ok(HttpResponse::Found()
                .append_header((
                    "Location",
                    format!("{}://{}/absolute-redirect/{}", scheme, host, n - 1),
                ))
                .body(""))
        } else {
            Ok(HttpResponse::Found()
                .append_header(("Location", format!("/relative-redirect/{}", n - 1)))
                .body(""))
        }
    }
}

pub async fn relative_redirect_handler(
    _req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();

    if n == 0 {
        return Err(actix_web::error::ErrorBadRequest(
            "n must be greater than 0",
        ));
    }

    if n == 1 {
        // Final redirect to /get
        Ok(HttpResponse::Found()
            .append_header(("Location", "/get"))
            .body(""))
    } else {
        // Relative redirect to the next step
        Ok(HttpResponse::Found()
            .append_header(("Location", format!("/relative-redirect/{}", n - 1)))
            .body(""))
    }
}

pub async fn absolute_redirect_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();

    if n == 0 {
        return Err(actix_web::error::ErrorBadRequest(
            "n must be greater than 0",
        ));
    }

    let host = req.connection_info().host().to_string();
    let scheme = if req.connection_info().scheme() == "https" {
        "https"
    } else {
        "http"
    };

    if n == 1 {
        // Final redirect to /get
        Ok(HttpResponse::Found()
            .append_header(("Location", format!("{}://{}/get", scheme, host)))
            .body(""))
    } else {
        // Absolute redirect to the next step
        Ok(HttpResponse::Found()
            .append_header((
                "Location",
                format!("{}://{}/absolute-redirect/{}", scheme, host, n - 1),
            ))
            .body(""))
    }
}

pub async fn redirect_to_handler_get(
    req: HttpRequest,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse> {
    let params = to_case_insensitive_map(&query);

    let url = params
        .get("url")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing required parameter: url"))?;

    // Browser requests get an interstitial page instead of an automatic redirect.
    // This prevents open-redirect abuse where phishers use /redirect-to?url=<malicious>
    // to redirect victims from the trusted httpcan.org domain.
    // Programmatic clients (curl, etc.) still receive the 302 for API compatibility.
    if is_browser_request(&req) {
        return Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .append_header(("X-Robots-Tag", "noindex"))
            .body(build_redirect_interstitial(url)));
    }

    let mut status_code = 302u16;
    if let Some(status_str) = params.get("status_code") {
        if let Ok(code) = status_str.parse::<u16>() {
            if (300..400).contains(&code) {
                status_code = code;
            }
        }
    }
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);

    // Match httpbin's exact UTF-8 encoding behavior
    let location_bytes = url.as_bytes();

    Ok(HttpResponse::build(status)
        .append_header(("Location", location_bytes))
        .body(""))
}

pub async fn redirect_to_handler(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse> {
    // Extract parameters from query string first (lower priority)
    let query_params = parse_query_params(req.query_string());

    // Extract parameters from request body based on content type
    let body_params = extract_body_params(&req, payload).await?;

    // Merge parameters with body taking priority over query
    let mut all_params = query_params;
    all_params.extend(body_params);

    // Convert to case-insensitive map
    let params = to_case_insensitive_hashmap(&all_params);

    let url = params
        .get("url")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing required parameter: url"))?;

    // Browser requests get an interstitial page instead of an automatic redirect.
    // See redirect_to_handler_get for rationale.
    if is_browser_request(&req) {
        return Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .append_header(("X-Robots-Tag", "noindex"))
            .body(build_redirect_interstitial(url)));
    }

    let mut status_code = 302u16;
    if let Some(status_str) = params.get("status_code") {
        if let Ok(code) = status_str.parse::<u16>() {
            if (300..400).contains(&code) {
                status_code = code;
            }
        }
    }
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);

    // Match httpbin's exact UTF-8 encoding behavior
    let location_bytes = url.as_bytes();

    Ok(HttpResponse::build(status)
        .append_header(("Location", location_bytes))
        .body(""))
}
