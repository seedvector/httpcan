use actix_web::{HttpRequest, Result};
use actix_multipart::Multipart;
use std::collections::HashMap;
use indexmap::IndexMap;
use futures_util::TryStreamExt;
use url::form_urlencoded;
use crate::{RequestInfo, GetRequestInfo};

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
            !proxy_headers.iter().any(|&proxy_header| lowercase_name == proxy_header)
        })
        .collect()
}

// Helper function to fix URL field in RequestInfo to include full URL
pub fn fix_request_info_url(req: &HttpRequest, request_info: &mut RequestInfo) {
    let connection_info = req.connection_info();
    let scheme = connection_info.scheme();
    let host = connection_info.host();
    let full_url = format!("{}://{}{}", scheme, host, req.uri());
    request_info.url = full_url;
}

// Helper function to extract GET request information (httpbin.org compatible)
pub fn extract_get_request_info(req: &HttpRequest) -> GetRequestInfo {
    let headers: HashMap<String, String> = req
        .headers()
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
        .collect();
    
    // Filter out reverse proxy and CDN headers
    let filtered_headers = filter_proxy_headers(headers);

    let args: HashMap<String, String> = req
        .query_string()
        .split('&')
        .filter_map(|pair| {
            if pair.is_empty() {
                return None;
            }
            let mut parts = pair.split('=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                (Some(key), None) => Some((key.to_string(), String::new())),
                _ => None,
            }
        })
        .collect();

    let connection_info = req.connection_info();
    let origin = connection_info.realip_remote_addr().unwrap_or("127.0.0.1").to_string();
    
    // Construct full URL including scheme and host
    let scheme = connection_info.scheme();
    let host = connection_info.host();
    let full_url = format!("{}://{}{}", scheme, host, req.uri());
    
    GetRequestInfo {
        args: sort_hashmap(args),
        headers: sort_hashmap(filtered_headers),
        origin,
        url: full_url,
    }
}

// Helper function to extract request information
pub fn extract_request_info(req: &HttpRequest, body: Option<&str>) -> RequestInfo {
    let headers: HashMap<String, String> = req
        .headers()
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
        .collect();
    
    // Filter out reverse proxy and CDN headers
    let filtered_headers = filter_proxy_headers(headers);

    let args: HashMap<String, String> = req
        .query_string()
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.split('=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                _ => None,
            }
        })
        .collect();

    let connection_info = req.connection_info();
    let origin = connection_info.realip_remote_addr().unwrap_or("127.0.0.1").to_string();
    
    // Parse form data based on content type
    let mut form_data = HashMap::new();
    let mut data_string = String::new();
    
    if let Some(body_str) = body {
        let content_type = req.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
            
        if content_type.to_lowercase().starts_with("application/x-www-form-urlencoded") {
            // Parse URL-encoded form data
            for (key, value) in form_urlencoded::parse(body_str.as_bytes()) {
                form_data.insert(key.to_string(), value.to_string());
            }
        } else if content_type.to_lowercase().starts_with("multipart/form-data") {
            // For multipart data, put raw data in data field as fallback
            // The proper multipart parsing should be done via extract_request_info_multipart
            data_string = body_str.to_string();
        } else {
            // For non-form data, put it in the data field
            data_string = body_str.to_string();
        }
    }
    
    RequestInfo {
        args: sort_hashmap(args),
        data: data_string,
        files: IndexMap::new(),
        form: sort_hashmap(form_data),
        headers: sort_hashmap(filtered_headers),
        json: body.and_then(|b| {
            if let Some(content_type) = req.headers().get("content-type")
                .and_then(|v| v.to_str().ok()) {
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
        user_agent: req.headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
    }
}

// Helper function to extract request information from multipart data
pub async fn extract_request_info_multipart(req: &HttpRequest, mut payload: Multipart) -> Result<RequestInfo> {
    let headers: HashMap<String, String> = req
        .headers()
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
        .collect();
    
    // Filter out reverse proxy and CDN headers
    let filtered_headers = filter_proxy_headers(headers);

    let args: HashMap<String, String> = req
        .query_string()
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.split('=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                _ => None,
            }
        })
        .collect();

    let connection_info = req.connection_info();
    let origin = connection_info.realip_remote_addr().unwrap_or("127.0.0.1").to_string();
    
    let mut form_data = HashMap::new();
    let mut files = HashMap::new();
    
    // Parse multipart data
    while let Some(mut field) = payload.try_next().await? {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition.get_name().map(|s| s.to_string());
        let filename = content_disposition.get_filename().map(|s| s.to_string());
        
        if let Some(name) = field_name {
            let mut data = Vec::new();
            
            // Read field data
            while let Some(chunk) = field.try_next().await? {
                data.extend_from_slice(&chunk);
            }
            
            if let Some(filename) = filename {
                // This is a file upload
                files.insert(
                    name,
                    format!("{} ({} bytes)", filename, data.len())
                );
            } else {
                // This is a regular form field
                if let Ok(value) = String::from_utf8(data) {
                    form_data.insert(name, value);
                }
            }
        }
    }
    
    Ok(RequestInfo {
        args: sort_hashmap(args),
        data: String::new(),
        files: sort_hashmap(files),
        form: sort_hashmap(form_data),
        headers: sort_hashmap(filtered_headers),
        json: None,
        method: req.method().to_string(),
        origin,
        url: req.uri().to_string(),
        user_agent: req.headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
    })
}
