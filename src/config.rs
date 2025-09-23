use clap::Parser;

/// Application configuration
#[derive(Clone)]
pub struct AppConfig {
    pub add_current_server: bool,
    pub exclude_headers: Vec<String>,
}

/// HTTPCan - HTTP testing service similar to httpbin.org
#[derive(Parser)]
#[command(name = "httpcan")]
#[command(about = "A simple HTTP request & response service", long_about = None)]
#[command(version)]
pub struct Args {
    /// Port number to listen on
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,
    
    /// Do not add current server to OpenAPI specification servers list
    #[arg(long)]
    pub no_current_server: bool,
    
    /// Exclude specific headers from responses. Comma-separated list of header keys, supports wildcard suffix matching (e.g., "foo, x-bar-*")
    #[arg(long)]
    pub exclude_headers: Option<String>,
}
