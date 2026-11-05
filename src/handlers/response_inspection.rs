use super::*;
use crate::handlers::utils::{generate_etag, http_date, parse_multi_value_header};
use serde_json::Value;
use std::collections::BTreeMap;

pub async fn cache_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    let if_modified_since = req.headers().get("If-Modified-Since");
    let if_none_match = req.headers().get("If-None-Match");

    // Generate dynamic values like httpbin
    let last_modified = http_date();
    let etag = generate_etag();

    if if_modified_since.is_some() || if_none_match.is_some() {
        // 304 response should include cache-related headers
        Ok(HttpResponse::NotModified()
            .append_header(("Last-Modified", last_modified))
            .append_header(("ETag", etag))
            .finish())
    } else {
        let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
        fix_request_info_url(&req, &mut request_info);
        Ok(HttpResponse::Ok()
            .append_header(("Last-Modified", last_modified))
            .append_header(("ETag", etag))
            .json(request_info))
    }
}

pub async fn cache_control_handler(
    req: HttpRequest,
    path: web::Path<u32>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let seconds = path.into_inner();
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);

    Ok(HttpResponse::Ok()
        .append_header(("Cache-Control", format!("public, max-age={}", seconds)))
        .json(request_info))
}

/// Compare a candidate ETag against a canonical (unquoted, no `W/`) value using
/// RFC 7232 weak comparison (for If-None-Match). `*` always matches.
fn etag_matches(candidate: &str, canonical: &str) -> bool {
    if candidate == "*" {
        return true;
    }
    candidate
        .strip_prefix("W/")
        .unwrap_or(candidate)
        .trim_matches('"')
        == canonical
}

pub async fn etag_handler(
    req: HttpRequest,
    path: web::Path<String>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let raw_etag = path.into_inner();
    let is_weak = raw_etag.starts_with("W/");
    let canonical = raw_etag.strip_prefix("W/").unwrap_or(&raw_etag);
    let canonical = canonical.trim_matches('"');
    let etag_header = if is_weak {
        format!("W/\"{}\"", canonical)
    } else {
        format!("\"{}\"", canonical)
    };

    let if_none_match_values = parse_multi_value_header(req.headers().get("If-None-Match"));
    let if_match_values = parse_multi_value_header(req.headers().get("If-Match"));

    // Check If-None-Match first (httpbin logic), using weak comparison (#400).
    if !if_none_match_values.is_empty() {
        if if_none_match_values
            .iter()
            .any(|v| etag_matches(v, canonical))
        {
            return Ok(HttpResponse::NotModified()
                .append_header(("ETag", etag_header.clone()))
                .finish());
        }
    } else if !if_match_values.is_empty()
        && !if_match_values.iter().any(|v| etag_matches(v, canonical))
    {
        return Ok(HttpResponse::PreconditionFailed().finish());
    }

    // Normal response with ETag
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    Ok(HttpResponse::Ok()
        .append_header(("ETag", etag_header))
        .json(request_info))
}

/// Parse query string to support multi-value parameters like httpbin
fn parse_multi_value_query_string(query_string: &str) -> BTreeMap<String, Vec<String>> {
    let mut params: BTreeMap<String, Vec<String>> = BTreeMap::new();

    if query_string.is_empty() {
        return params;
    }

    for pair in query_string.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let decoded_key = urlencoding::decode(key)
                .unwrap_or_else(|_| key.into())
                .to_string();
            let decoded_value = urlencoding::decode(value)
                .unwrap_or_else(|_| value.into())
                .to_string();
            params.entry(decoded_key).or_default().push(decoded_value);
        } else if !pair.is_empty() {
            // Handle keys without values
            let decoded_key = urlencoding::decode(pair)
                .unwrap_or_else(|_| pair.into())
                .to_string();
            params.entry(decoded_key).or_default().push(String::new());
        }
    }

    params
}

pub async fn response_headers_get_handler(
    req: HttpRequest,
    _query: web::Query<HashMap<String, String>>, // Keep for compatibility but use manual parsing
) -> Result<HttpResponse> {
    // Parse query string manually to support multi-value parameters
    let multi_value_params = parse_multi_value_query_string(req.query_string());
    // #655: ?body=<text> (and optional ?status=<code>) return a custom body
    // instead of the JSON header dump. Other query params still become headers.
    if let Some(body) = multi_value_params.get("body").and_then(|v| v.last()) {
        let status = multi_value_params
            .get("status")
            .and_then(|v| v.last())
            .and_then(|s| s.parse::<u16>().ok())
            .and_then(|c| StatusCode::from_u16(c).ok())
            .unwrap_or(StatusCode::OK);
        let mut builder = HttpResponse::build(status);
        for (key, values) in &multi_value_params {
            if matches!(key.as_str(), "body" | "status") {
                continue;
            }
            for value in values {
                builder.append_header((key.as_str(), value.as_str()));
            }
        }
        return Ok(builder.body(body.clone()));
    }

    // Prepare the headers map for the JSON response
    let mut headers_map: BTreeMap<String, Value> = BTreeMap::new();
    for (key, values) in &multi_value_params {
        if values.len() == 1 {
            headers_map.insert(key.clone(), Value::String(values[0].clone()));
        } else {
            headers_map.insert(
                key.clone(),
                Value::Array(values.iter().map(|v| Value::String(v.clone())).collect()),
            );
        }
    }

    // Add standard headers that would be set by the framework
    headers_map.insert(
        "Content-Type".to_string(),
        Value::String("application/json".to_string()),
    );

    // The body reports its own Content-Length, so re-serialize until the
    // value matches the body size (like httpbin does). This settles within a
    // couple of rounds; the cap is just a safety net.
    const MAX_ITERATIONS: usize = 10;
    let mut json_string = serde_json::to_string(&headers_map)?;
    for _ in 0..MAX_ITERATIONS {
        let content_length = json_string.len().to_string();
        if headers_map.get("Content-Length") == Some(&Value::String(content_length.clone())) {
            break;
        }
        headers_map.insert("Content-Length".to_string(), Value::String(content_length));
        json_string = serde_json::to_string(&headers_map)?;
    }

    let mut response_builder = HttpResponse::Ok();

    // Add query parameters as actual response headers
    for (key, values) in &multi_value_params {
        for value in values {
            response_builder.append_header((key.as_str(), value.as_str()));
        }
    }

    Ok(response_builder
        .content_type("application/json")
        .body(json_string))
}

pub async fn response_headers_post_handler(
    req: HttpRequest,
    _query: web::Query<HashMap<String, String>>, // Keep for compatibility but use manual parsing
    _body: String,
) -> Result<HttpResponse> {
    // Use the same logic as GET handler - httpbin treats GET and POST identically for this endpoint
    response_headers_get_handler(req, _query).await
}
