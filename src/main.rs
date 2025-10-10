use clap::Parser;
use httpcan::{HttpCanServer, ServerConfig};

mod config;
use config::Args;


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Parse command line arguments
    let args = Args::parse();
    
    // Parse exclude headers
    let exclude_headers: Vec<String> = args.exclude_headers
        .map(|headers_str| {
            headers_str
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Create server configuration
    let config = ServerConfig::new()
        .port(args.port)
        .add_current_server(!args.no_current_server)
        .exclude_headers(exclude_headers);

    // Create and run the server
    HttpCanServer::with_config(config)
        .run()
        .await
}
