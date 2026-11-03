use super::*;
use crate::handlers::page::{html_escape, icon, page_shell, warn_icon};
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
        let field_name = content_disposition
            .and_then(|cd| cd.get_name())
            .map(|s| s.to_string());

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

/// Build an HTML interstitial page for external redirect-to requests.
///
/// Instead of returning a 302 that silently redirects browser users to an
/// arbitrary URL, return a 200 HTML page that shows the destination and
/// requires a click. Programmatic clients (curl, etc.) still get the 302.
///
/// The destination host is pulled out and shown prominently so a lookalike
/// domain is easy to spot; the safe action ("Go back") is the primary button.
/// Non-http(s) URLs get a disabled continue button and no link at all.
fn build_redirect_interstitial(url: &str) -> String {
    let safe = is_safe_url_scheme(url);
    let host = if safe {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
    } else {
        None
    };
    let globe = icon(
        "<circle cx=\"12\" cy=\"12\" r=\"10\"/><path d=\"M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20\"/><path d=\"M2 12h20\"/>",
    );
    let arrow = icon("<path d=\"M7 7h10v10\"/><path d=\"M7 17 17 7\"/>");

    let (lead, pill, dest_title, continue_btn, explanation) = if safe {
        let host_label = host.as_deref().unwrap_or("external site");
        (
            "You&rsquo;re about to leave httpcan for:",
            "<span class=\"pill pill-warn\">Leaving httpcan</span>",
            html_escape(host_label),
            format!(
                "<a class=\"btn btn-outline\" href=\"{}\" rel=\"noopener noreferrer\">Continue{}</a>",
                html_escape(url),
                arrow
            ),
            "This link was made with httpcan&rsquo;s <code>/redirect-to</code> endpoint, which can point anywhere. \
httpcan doesn&rsquo;t control the destination &mdash; check the domain above and only continue if you trust it."
                .to_string(),
        )
    } else {
        (
            "This link can&rsquo;t be opened:",
            "<span class=\"pill pill-err\">Blocked</span>",
            "Unsupported link".to_string(),
            "<span class=\"btn btn-outline\" aria-disabled=\"true\">Continue</span>".to_string(),
            "Only <code>http://</code> and <code>https://</code> destinations can be opened from this page."
                .to_string(),
        )
    };

    let card = format!(
        "<div class=\"rim\"><span>External link</span>{pill}</div>\n\
<div class=\"body\">\n\
<p class=\"lead\">{lead}</p>\n\
<div class=\"dest\"><div class=\"dest-host\">{globe}{dest_title}</div><p class=\"dest-url\">{url}</p></div>\n\
<div class=\"notice\">{warn}<p>{explanation}</p></div>\n\
</div>\n\
<div class=\"actions\">\n\
{continue_btn}\n\
<a class=\"btn btn-primary\" href=\"/\">Go back</a>\n\
</div>",
        url = html_escape(url),
        warn = warn_icon(),
    );
    page_shell(
        "External Link - HTTPCan",
        "Redirect check",
        &card,
        &[("/", "httpcan home"), ("/#redirects", "About redirects")],
    )
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
