use actix_files::NamedFile;
use actix_web::HttpResponse;

use actix_multipart::Multipart;
use actix_web::web::BytesMut;
use actix_web::{web, HttpRequest, Result};
use base64::{engine::general_purpose, Engine as _};
use futures_util::{StreamExt, TryStreamExt};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::path::PathBuf;
use url::form_urlencoded;
use urlencoding;

/// Collect request headers into a map, joining multiple values that share the
/// same header name with `", "` per RFC 7230 §3.2.2. This preserves duplicate
/// header values instead of collapsing them to the last value (httpbin #355).
pub fn collect_request_headers(req: &HttpRequest) -> HashMap<String, String> {
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in req.headers().iter() {
        let value_str = value.to_str().unwrap_or("");
        headers
            .entry(name.to_string())
            .and_modify(|existing| {
                existing.push_str(", ");
                existing.push_str(value_str);
            })
            .or_insert_with(|| value_str.to_string());
    }
    headers
}
#[derive(Serialize, Deserialize)]
pub struct RequestInfo {
    pub args: BTreeMap<String, Value>,
    pub data: String,
    pub files: IndexMap<String, Value>,
    pub form: IndexMap<String, Value>,
    pub headers: IndexMap<String, String>,
    pub json: Option<Value>,
    pub method: String,
    pub origin: String,
    pub url: String,
}

// Simplified response structure for GET requests (httpbin.org compatible)
#[derive(Serialize, Deserialize)]
pub struct GetRequestInfo {
    pub args: BTreeMap<String, Value>,
    pub headers: IndexMap<String, String>,
    pub origin: String,
    pub url: String,
}

// HTTPBin compatible response structure for POST/PUT/PATCH/DELETE requests
#[derive(Serialize, Deserialize)]
pub struct HttpMethodsRequestInfo {
    pub args: BTreeMap<String, Value>,
    pub data: String,
    pub files: IndexMap<String, Value>,
    pub form: IndexMap<String, Value>,
    pub headers: IndexMap<String, String>,
    pub json: Option<Value>,
    pub origin: String,
    pub url: String,
}

// Helper function to check if data appears to be text content
fn is_text_content(data: &[u8]) -> bool {
    // Check for null bytes (common in binary files)
    if data.contains(&0) {
        return false;
    }

    // Check if content is valid UTF-8
    std::str::from_utf8(data).is_ok()
}

// Helper function to format file content for display
fn format_file_content(_filename: &str, data: &[u8]) -> String {
    if is_text_content(data) {
        // For text files, return the content directly
        match std::str::from_utf8(data) {
            Ok(text) => text.to_string(),
            Err(_) => "[Invalid UTF-8]".to_string(),
        }
    } else {
        // For binary files, return base64 encoding directly
        general_purpose::STANDARD.encode(data)
    }
}

// Helper function to get static directory path relative to executable
pub fn get_static_path() -> PathBuf {
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

/// The user-overridable assets and their canonical URLs. A file with one of
/// these names placed in the static assets directory replaces the built-in
/// default at its canonical URL; every other file in the directory is only
/// reachable at `/static/<name>` (or `/<name>` when it does not collide with
/// an API route).
pub const OVERRIDABLE_ASSETS: &[&str] = &[
    "openapi.json",
    "favicon.png",
    "index.html",
    "robots.txt",
    "sitemap.xml",
];

/// User override layer: if `static/<name>` exists, open it as a [`NamedFile`].
/// Checked per request, so dropping a file into the directory takes effect
/// without a restart. `Err(io)` other than "not found" is treated as "no
/// override" — the built-in default is still served.
pub fn static_override(config: &crate::AppConfig, name: &str) -> Option<NamedFile> {
    NamedFile::open(config.static_path.join(name)).ok()
}

pub fn override_response(
    file: NamedFile,
    content_type: &'static str,
    req: &HttpRequest,
) -> Result<HttpResponse> {
    let ct: mime::Mime = content_type.parse().expect("valid MIME type literal");
    Ok(file.set_content_type(ct).into_response(req))
}

// Helper function to sort HashMap by keys and return IndexMap
pub fn sort_hashmap(map: HashMap<String, String>) -> IndexMap<String, String> {
    let mut keys: Vec<_> = map.keys().cloned().collect();
    keys.sort();
    let mut sorted_map = IndexMap::new();
    for key in keys {
        if let Some(value) = map.get(&key) {
            sorted_map.insert(key, value.clone());
        }
    }
    sorted_map
}

// Helper function to sort HashMap with Value by keys and return IndexMap
pub fn sort_hashmap_value(map: HashMap<String, Value>) -> IndexMap<String, Value> {
    let mut keys: Vec<_> = map.keys().cloned().collect();
    keys.sort();
    let mut sorted_map = IndexMap::new();
    for key in keys {
        if let Some(value) = map.get(&key) {
            sorted_map.insert(key, value.clone());
        }
    }
    sorted_map
}

// Helper function to match header name against pattern (supports wildcard suffix matching)
fn header_matches_pattern(header_name: &str, pattern: &str) -> bool {
    let header_lower = header_name.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    if pattern_lower.ends_with('*') {
        // Wildcard suffix matching
        let prefix = &pattern_lower[..pattern_lower.len() - 1];
        header_lower.starts_with(prefix)
    } else {
        // Exact matching
        header_lower == pattern_lower
    }
}

// Enhanced header filtering function that supports both proxy filtering and custom exclusions
pub fn filter_headers(
    headers: HashMap<String, String>,
    exclude_patterns: &[String],
) -> HashMap<String, String> {
    // First apply proxy header filtering
    let proxy_filtered = filter_proxy_headers(headers);

    // Then apply custom exclusions
    proxy_filtered
        .into_iter()
        .filter(|(name, _)| {
            !exclude_patterns
                .iter()
                .any(|pattern| header_matches_pattern(name, pattern))
        })
        .collect()
}

// Helper function to filter out reverse proxy and CDN headers
// Uses conservative filtering - only removes headers that are almost certainly from infrastructure
pub fn filter_proxy_headers(headers: HashMap<String, String>) -> HashMap<String, String> {
    // Conservative list of headers that are almost certainly added by infrastructure
    // We only filter headers that are very unlikely to be sent intentionally by users
    let proxy_headers: Vec<&str> = vec![
        // Nginx headers
        "x-real-ip",
        "x-forwarded-for",
        "x-forwarded-proto",
        "x-forwarded-host",
        "x-forwarded-port",
        "x-original-uri",
        "x-original-url",
        "x-forwarded-ssl",
        "x-forwarded-scheme",
        "x-nginx-proxy",
        // Cloudflare headers
        "cf-ray",
        "cf-cache-status",
        "cf-connecting-ip",
        "cf-ipcountry",
        "cf-visitor",
        "cf-request-id",
        "cf-worker",
        "cf-warp-tag-id",
        "cf-edge-cache",
        "cf-cache-tag",
        "cf-railgun",
        "cdn-loop",
        // AWS CloudFront headers
        "cloudfront-viewer-address",
        "cloudfront-viewer-asn",
        "cloudfront-viewer-country",
        "cloudfront-viewer-city",
        "cloudfront-viewer-country-name",
        "cloudfront-viewer-country-region",
        "cloudfront-viewer-country-region-name",
        "cloudfront-viewer-latitude",
        "cloudfront-viewer-longitude",
        "cloudfront-viewer-metro-code",
        "cloudfront-viewer-postal-code",
        "cloudfront-viewer-time-zone",
        "cloudfront-viewer-header-order",
        "cloudfront-viewer-header-count",
        "cloudfront-forwarded-proto",
        "cloudfront-is-android-viewer",
        "cloudfront-is-desktop-viewer",
        "cloudfront-is-ios-viewer",
        "cloudfront-is-mobile-viewer",
        "cloudfront-is-smarttv-viewer",
        "cloudfront-is-tablet-viewer",
        "x-amz-cf-id",
        "x-amz-cf-pop",
        "x-amz-cloudfront-id",
        // AWS Load Balancer headers (ALB/ELB)
        "x-amzn-trace-id",
        "x-amzn-requestid",
        "x-amzn-request-id",
        "x-amz-request-id",
        "x-amzn-elb-id",
        "x-amzn-lb-id",
        // Google Cloud Platform (GCP) headers
        "x-cloud-trace-context",
        "x-goog-trace",
        "x-goog-request-id",
        "x-google-trace",
        "x-google-request-id",
        "x-gfe-request-trace",
        "x-gfe-response-code-details-trace",
        "x-goog-iap-jwt-assertion",
        "x-forwarded-for-original",
        "x-appengine-city",
        "x-appengine-citylatlong",
        "x-appengine-country",
        "x-appengine-region",
        "x-appengine-request-id",
        "x-appengine-datacenter",
        "x-appengine-default-namespace",
        "x-appengine-https",
        "x-appengine-request-log-id",
        "x-appengine-user-ip",
        "x-appengine-user-id",
        "x-appengine-user-email",
        "x-appengine-user-nickname",
        "x-appengine-auth-domain",
        "x-appengine-cron",
        "x-appengine-taskname",
        "x-appengine-queuename",
        "x-appengine-taskretrycount",
        "x-appengine-taskexecutioncount",
        "x-appengine-tasketa",
        // Microsoft Azure headers
        "x-azure-ref",
        "x-azure-requestid",
        "x-azure-request-id",
        "x-ms-request-id",
        "x-ms-correlation-request-id",
        "x-ms-routing-request-id",
        "x-ms-exchange-crosstenant-originalauthenticatedcontext",
        "x-ms-exchange-crosstenant-fromentityheader",
        "x-ms-exchange-crosstenant-id",
        "x-azure-fdid",
        "x-azure-socketip",
        "x-fd-healthprobe",
        "x-azure-clientip",
        "x-azure-ref-originshield",
        "x-cache-remote",
        "x-p3p",
        "x-msedge-ref",
        "x-azure-appliedaccesspolicy",
        "x-azure-appliedpolicy",
    ];

    headers
        .into_iter()
        .filter(|(name, _)| {
            let lowercase_name = name.to_lowercase();
            !proxy_headers
                .iter()
                .any(|&proxy_header| lowercase_name == proxy_header)
        })
        .collect()
}

/// Resolve the scheme (`http`/`https`) used for the SEO-facing
/// self-identification surfaces: the homepage canonical link, `sitemap.xml`,
/// and the `Sitemap:` directive in `robots.txt`.
///
/// Honors [`crate::config::SchemeOverride`] so deployments behind a
/// TLS-terminating reverse proxy/CDN that doesn't forward
/// `X-Forwarded-Proto` correctly can pin the scheme, instead of trusting a
/// per-request header that may be missing, wrong, or spoofable.
///
/// Everything else — copy-curl examples on the homepage, the OpenAPI
/// "current server" entry, `/.well-known/api-catalog`, and endpoints that
/// intentionally mirror the exact request the client made (e.g. `/get`,
/// `/anything`, `/absolute-redirect`) — should keep using
/// `req.connection_info().scheme()` directly, so a visitor always gets back
/// URLs that actually work for the request they made.
pub fn resolved_scheme(req: &HttpRequest, config: &crate::AppConfig) -> String {
    match config.canonical_scheme.fixed() {
        Some(scheme) => scheme.to_string(),
        None => req.connection_info().scheme().to_string(),
    }
}

/// Resolve the `scheme://host` origin for self-referential absolute URLs.
/// See [`resolved_scheme`].
pub fn resolved_base(req: &HttpRequest, config: &crate::AppConfig) -> String {
    format!(
        "{}://{}",
        resolved_scheme(req, config),
        req.connection_info().host()
    )
}

// Helper function to fix URL field in RequestInfo to include full URL
pub fn fix_request_info_url(req: &HttpRequest, request_info: &mut RequestInfo) {
    let connection_info = req.connection_info();
    let scheme = connection_info.scheme();
    let host = connection_info.host();
    let full_url = format!("{}://{}{}", scheme, host, req.uri());
    request_info.url = full_url;
}

// Helper function to convert RequestInfo to HTTPBin compatible format
pub fn to_http_methods_format(request_info: RequestInfo) -> HttpMethodsRequestInfo {
    HttpMethodsRequestInfo {
        args: request_info.args,
        data: request_info.data,
        files: request_info.files,
        form: request_info.form,
        headers: request_info.headers,
        json: request_info.json,
        origin: request_info.origin,
        url: request_info.url,
    }
}

// Helper function to extract GET request information (httpbin.org compatible)
pub fn extract_get_request_info(req: &HttpRequest, exclude_patterns: &[String]) -> GetRequestInfo {
    let headers = collect_request_headers(req);

    // Filter out reverse proxy and CDN headers, plus custom exclusions
    let filtered_headers = filter_headers(headers, exclude_patterns);

    let args = parse_multi_value_query_string(req.query_string());

    let connection_info = req.connection_info();
    let origin = connection_info
        .realip_remote_addr()
        .unwrap_or("127.0.0.1")
        .to_string();

    // Construct full URL including scheme and host
    let scheme = connection_info.scheme();
    let host = connection_info.host();
    let full_url = format!("{}://{}{}", scheme, host, req.uri());

    GetRequestInfo {
        args,
        headers: sort_hashmap(filtered_headers),
        origin,
        url: full_url,
    }
}

/// Parse query string to support multi-value parameters with robust UTF-8 handling
fn parse_multi_value_query_string(query_string: &str) -> BTreeMap<String, Value> {
    let mut params: BTreeMap<String, Vec<String>> = BTreeMap::new();

    if query_string.is_empty() {
        return BTreeMap::new();
    }

    for pair in query_string.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            // Handle both encoded and raw UTF-8 characters
            let decoded_key = if key.contains('%') {
                urlencoding::decode(key)
                    .unwrap_or_else(|_| key.into())
                    .to_string()
            } else {
                key.to_string()
            };
            let decoded_value = if value.contains('%') {
                urlencoding::decode(value)
                    .unwrap_or_else(|_| value.into())
                    .to_string()
            } else {
                value.to_string()
            };
            params.entry(decoded_key).or_default().push(decoded_value);
        } else if !pair.is_empty() {
            // Handle keys without values
            let decoded_key = if pair.contains('%') {
                urlencoding::decode(pair)
                    .unwrap_or_else(|_| pair.into())
                    .to_string()
            } else {
                pair.to_string()
            };
            params.entry(decoded_key).or_default().push(String::new());
        }
    }

    // Convert to BTreeMap<String, Value> - single values as strings, multiple as arrays
    params
        .into_iter()
        .map(|(key, values)| {
            let value = if values.len() == 1 {
                Value::String(values.into_iter().next().unwrap())
            } else {
                Value::Array(values.into_iter().map(Value::String).collect())
            };
            (key, value)
        })
        .collect()
}

/// Parse form data to support multi-value parameters (similar to query string parsing)
fn parse_multi_value_form_data(form_data: &str) -> BTreeMap<String, Value> {
    let mut params: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // Use form_urlencoded to properly decode the form data
    for (key, value) in form_urlencoded::parse(form_data.as_bytes()) {
        params
            .entry(key.to_string())
            .or_default()
            .push(value.to_string());
    }

    // Convert to BTreeMap<String, Value> - single values as strings, multiple as arrays
    params
        .into_iter()
        .map(|(key, values)| {
            let value = if values.len() == 1 {
                Value::String(values.into_iter().next().unwrap())
            } else {
                Value::Array(values.into_iter().map(Value::String).collect())
            };
            (key, value)
        })
        .collect()
}

// Helper function to extract request information
pub fn extract_request_info(
    req: &HttpRequest,
    body: Option<&str>,
    exclude_patterns: &[String],
) -> RequestInfo {
    let headers = collect_request_headers(req);

    // Filter out reverse proxy and CDN headers, plus custom exclusions
    let filtered_headers = filter_headers(headers, exclude_patterns);

    let args = parse_multi_value_query_string(req.query_string());

    let connection_info = req.connection_info();
    let origin = connection_info
        .realip_remote_addr()
        .unwrap_or("127.0.0.1")
        .to_string();

    // Parse form data based on content type
    let mut form_data_values: BTreeMap<String, Value> = BTreeMap::new();
    let mut data_string = String::new();

    if let Some(body_str) = body {
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if content_type
            .to_lowercase()
            .starts_with("application/x-www-form-urlencoded")
        {
            // Parse URL-encoded form data with support for duplicate keys
            form_data_values = parse_multi_value_form_data(body_str);
        } else if content_type
            .to_lowercase()
            .starts_with("multipart/form-data")
        {
            // For multipart data, put raw data in data field as fallback
            // The proper multipart parsing should be done via extract_request_info_multipart
            data_string = body_str.to_string();
        } else {
            // For non-form data, put it in the data field
            data_string = body_str.to_string();
        }
    }

    RequestInfo {
        args,
        data: data_string,
        files: IndexMap::new(),
        form: sort_hashmap_value(form_data_values.into_iter().collect()),
        headers: sort_hashmap(filtered_headers),
        json: body.and_then(|b| {
            if let Some(content_type) = req
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
            {
                if content_type.starts_with("application/json") {
                    serde_json::from_str(b).ok()
                } else {
                    None
                }
            } else {
                None
            }
        }),
        method: req.method().to_string(),
        origin,
        url: req.uri().to_string(),
    }
}

// Helper function to extract request information from multipart data
pub async fn extract_request_info_multipart(
    req: &HttpRequest,
    mut payload: Multipart,
    exclude_patterns: &[String],
) -> Result<RequestInfo> {
    let headers = collect_request_headers(req);

    // Filter out reverse proxy and CDN headers, plus custom exclusions
    let filtered_headers = filter_headers(headers, exclude_patterns);

    let args = parse_multi_value_query_string(req.query_string());

    let origin = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("127.0.0.1")
        .to_string();

    let mut form_data: HashMap<String, Vec<Value>> = HashMap::new();
    let mut files: HashMap<String, Vec<Value>> = HashMap::new();

    // Parse multipart data
    while let Some(mut field) = payload.try_next().await? {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition
            .and_then(|cd| cd.get_name())
            .map(|s| s.to_string());
        let filename = content_disposition
            .and_then(|cd| cd.get_filename())
            .map(|s| s.to_string());
        // Capture the per-part Content-Type (httpbin #722).
        let part_content_type = field.content_type().map(|m| m.to_string());

        if let Some(name) = field_name {
            let mut data = Vec::new();

            // Read field data
            while let Some(chunk) = field.try_next().await? {
                data.extend_from_slice(&chunk);
            }

            if let Some(filename) = filename {
                // File upload - include filename, content_type, and content (httpbin #722).
                let file_value = serde_json::json!({
                    "filename": filename,
                    "content_type": part_content_type,
                    "content": format_file_content(&filename, &data),
                });
                files.entry(name).or_default().push(file_value);
            } else {
                // Regular form field - parse JSON parts into objects (httpbin #693),
                // otherwise keep the raw string value.
                if let Ok(text) = String::from_utf8(data) {
                    let is_json = part_content_type
                        .as_deref()
                        .map(|c| c.to_lowercase().starts_with("application/json"))
                        .unwrap_or(false);
                    let value = if is_json {
                        serde_json::from_str(&text).unwrap_or(Value::String(text))
                    } else {
                        Value::String(text)
                    };
                    form_data.entry(name).or_default().push(value);
                }
            }
        }
    }

    // Convert each Vec<Value> to a single Value or array.
    let files_map: HashMap<String, Value> = files
        .into_iter()
        .map(|(key, values)| {
            let value = if values.len() == 1 {
                values.into_iter().next().unwrap()
            } else {
                Value::Array(values)
            };
            (key, value)
        })
        .collect();

    let form_map: HashMap<String, Value> = form_data
        .into_iter()
        .map(|(key, values)| {
            let value = if values.len() == 1 {
                values.into_iter().next().unwrap()
            } else {
                Value::Array(values)
            };
            (key, value)
        })
        .collect();

    Ok(RequestInfo {
        args,
        data: String::new(),
        files: sort_hashmap_value(files_map),
        form: sort_hashmap_value(form_map),
        headers: sort_hashmap(filtered_headers),
        json: None,
        method: req.method().to_string(),
        origin,
        url: req.uri().to_string(),
    })
}

/// Parse multi-value header (like If-None-Match) into a vector of values
/// Handles comma-separated values and quoted strings properly
pub fn parse_multi_value_header(
    header_value: Option<&actix_web::http::header::HeaderValue>,
) -> Vec<String> {
    if let Some(value) = header_value {
        if let Ok(value_str) = value.to_str() {
            let mut values = Vec::new();
            let mut current = String::new();
            let mut in_quotes = false;
            let chars = value_str.chars();

            for ch in chars {
                match ch {
                    '"' => {
                        in_quotes = !in_quotes;
                        current.push(ch);
                    }
                    ',' if !in_quotes => {
                        let trimmed = current.trim().to_string();
                        if !trimmed.is_empty() {
                            values.push(trimmed);
                        }
                        current.clear();
                    }
                    _ => {
                        current.push(ch);
                    }
                }
            }

            // Add the last value
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                values.push(trimmed);
            }

            values
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    }
}

/// Generate HTTP date string (RFC 7231 format)
pub fn http_date() -> String {
    use chrono::Utc;
    let now = Utc::now();
    now.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

/// Generate a random ETag value
pub fn generate_etag() -> String {
    use uuid::Uuid;
    let uuid = Uuid::new_v4();
    format!("\"{}\"", uuid.simple())
}

/// Universal handler for processing request payloads with multipart and regular body support
/// This reduces code duplication across multiple handlers
pub async fn process_request_payload(
    req: &HttpRequest,
    payload: web::Payload,
    exclude_headers: &[String],
    path_param: Option<String>,
) -> Result<RequestInfo> {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let mut request_info = if content_type
        .to_lowercase()
        .starts_with("multipart/form-data")
    {
        let multipart = Multipart::new(req.headers(), payload);
        match extract_request_info_multipart(req, multipart, exclude_headers).await {
            Ok(info) => info,
            Err(_) => extract_request_info(req, None, exclude_headers),
        }
    } else {
        let mut body = BytesMut::new();
        let mut payload = payload;
        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            body.extend_from_slice(&chunk);
        }

        let body_string = String::from_utf8_lossy(&body);
        extract_request_info(req, Some(&body_string), exclude_headers)
    };

    fix_request_info_url(req, &mut request_info);

    // Add path parameter if provided
    if let Some(path) = path_param {
        request_info
            .args
            .insert("anything".to_string(), serde_json::Value::String(path));
    }

    Ok(request_info)
}
