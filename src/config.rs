use clap::Parser;

/// Default maximum number of bytes returned by `/bytes` and `/stream-bytes`.
/// Matches httpbin's 100KB cap (httpbin #594).
pub const DEFAULT_MAX_BYTES: usize = 100 * 1024;

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

    /// Maximum bytes returned by `/bytes/{n}` and `/stream-bytes/{n}`. Requests exceeding this return a 404 instead of silently truncating.
    #[arg(long, default_value_t = DEFAULT_MAX_BYTES)]
    pub max_bytes: usize,
}
