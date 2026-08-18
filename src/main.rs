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

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

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

    // Parse the OAuth2 client registry: "id:secret,id2:secret2". Malformed
    // entries abort with exit code 2 (same contract as clap's bad values).
    let oauth2_clients = args.oauth2_clients.map(|spec| {
        spec.split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| match entry.split_once(':') {
                Some((id, secret)) => (id.trim().to_string(), secret.trim().to_string()),
                None => {
                    eprintln!("invalid --oauth2-clients entry (expected ID:SECRET): {entry}");
                    std::process::exit(2);
                }
            })
            .collect()
    });

    // Create server configuration
    let mut config = ServerConfig::new()
        .port(args.port)
        .add_current_server(args.openapi_servers.add_current_server())
        .exclude_headers(exclude_headers)
        .max_bytes(args.max_bytes)
        .canonical_scheme(args.canonical_scheme)
        .oauth2_clients(oauth2_clients);
    if let Some(dir) = args.static_dir {
        config = config.static_dir(dir);
    }

    // Create and run the server
    HttpCanServer::with_config(config).run().await
}
