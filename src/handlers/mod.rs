use actix_web::{
    web, HttpRequest, HttpResponse, Result,
    http::StatusCode,
    cookie::Cookie,
};
use actix_web_httpauth::extractors::{basic::BasicAuth, bearer::BearerAuth};
use actix_multipart::Multipart;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use futures_util::TryStreamExt;
use tokio::time::sleep;
use rand::Rng;
use base64::{Engine as _, engine::general_purpose};
use uuid::Uuid;
use url::form_urlencoded;
use crate::{RequestInfo, GetRequestInfo};

pub mod http_methods;
pub mod anything;
pub mod auth;
pub mod response_formats;
pub mod dynamic_data;
pub mod redirects;
pub mod request_inspection;
pub mod response_inspection;
pub mod cookies;
pub mod images;
pub mod status;

pub use http_methods::*;
pub use anything::*;
pub use auth::*;
pub use response_formats::*;
pub use dynamic_data::*;
pub use redirects::*;
pub use request_inspection::*;
pub use response_inspection::*;
pub use cookies::*;
pub use images::*;
pub use status::*;

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
        args,
        headers,
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
        args,
        data: data_string,
        files: HashMap::new(),
        form: form_data,
        headers,
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
        args,
        data: String::new(),
        files,
        form: form_data,
        headers,
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
