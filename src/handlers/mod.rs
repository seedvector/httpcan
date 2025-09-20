use actix_web::{
    web, HttpRequest, HttpResponse, Result,
    http::StatusCode,
    cookie::Cookie,
};
use actix_web_httpauth::extractors::{basic::BasicAuth, bearer::BearerAuth};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use futures_util::StreamExt;
use tokio::time::sleep;
use rand::Rng;
use base64::{Engine as _, engine::general_purpose};
use uuid::Uuid;
use crate::RequestInfo;

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
    
    RequestInfo {
        args,
        data: body.unwrap_or("").to_string(),
        files: HashMap::new(),
        form: HashMap::new(),
        headers,
        json: body.and_then(|b| serde_json::from_str(b).ok()),
        method: req.method().to_string(),
        origin,
        url: req.uri().to_string(),
        user_agent: req.headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
    }
}
