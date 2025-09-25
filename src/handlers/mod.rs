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
use tokio::time::sleep;
use rand::Rng;
use base64::{Engine as _, engine::general_purpose};
use uuid::Uuid;
use crate::AppConfig;

pub mod utils;
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
pub mod openapi;
pub mod sse;
pub mod echo;

pub use utils::*;
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
pub use openapi::*;
pub use sse::*;
pub use echo::*;

