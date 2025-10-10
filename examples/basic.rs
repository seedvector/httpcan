//! Basic example showing how to use HTTPCan as a library

use httpcan::{HttpCanServer, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Method 1: Using default configuration
    println!("Starting HTTPCan server with default configuration...");
    
    // Create server with default settings (port 8080, all features enabled)
    let _server = HttpCanServer::new();
    
    // Method 2: Using builder pattern
    let server = HttpCanServer::new()
        .port(3000)
        .host("127.0.0.1")
        .add_current_server(true)
        .exclude_header("x-internal-header")
        .exclude_header("x-debug-*");
    
    // Method 3: Using ServerConfig
    let config = ServerConfig::new()
        .port(3000)
        .host("127.0.0.1")
        .add_current_server(true)
        .exclude_headers(vec![
            "x-internal-header".to_string(),
            "x-debug-*".to_string(),
        ]);
    
    let _server_with_config = HttpCanServer::with_config(config);
    
    // Start the server
    server.run().await?;
    
    Ok(())
}
