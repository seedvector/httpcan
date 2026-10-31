//! Integration-style tests that drive the full app through `create_app`,
//! one module per endpoint family. Shared helpers live here.

use super::*;
use actix_web::{http::StatusCode, test};
use std::io::Read;

fn cfg() -> ServerConfig {
    ServerConfig::default()
}

mod auth;
mod cookies;
mod dynamic_data;
mod echo;
mod images;
mod inspection;
mod llm;
mod middleware;
mod oauth2;
mod observability;
mod openapi;
mod redirects;
mod response_formats;
mod root;
mod static_assets;
mod status;
mod streaming;
