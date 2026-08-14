use clap::Parser;
use httpcan::config::Args;
use httpcan::{HttpCanServer, ServerConfig};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load a `.env` file from the current directory into the process
    // environment, if present, before anything reads env vars (CLI flag
    // defaults below, and env_logger's RUST_LOG). Silently does nothing when
    // no `.env` file exists, so it's a no-op for Docker/systemd deployments
    // that already inject real environment variables.
    dotenvy::dotenv().ok();

    env_logger::init();

    // Parse command line arguments
    let args = Args::parse();

    // Parse exclude headers
    let exclude_headers: Vec<String> = args
        .exclude_headers
        .map(|headers_str| {
            headers_str
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Create server configuration
    let mut config = ServerConfig::new()
        .port(args.port)
        .add_current_server(args.openapi_servers.add_current_server())
        .exclude_headers(exclude_headers)
        .max_bytes(args.max_bytes)
        .canonical_scheme(args.canonical_scheme);
    if let Some(dir) = args.static_dir {
        config = config.static_dir(dir);
    }

    // Create and run the server
    HttpCanServer::with_config(config).run().await
}
