# HTTPCan Library Usage

HTTPCan can now be used as a Rust library, allowing you to start HTTPCan servers programmatically in your code.

## Using as a Library

Add HTTPCan as a dependency in your `Cargo.toml`:

```toml
[dependencies]
httpcan = { path = "path/to/httpcan" }
# Or if published to crates.io:
# httpcan = "0.5.1"
```

### Basic Usage

```rust
use httpcan::HttpCanServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Start server with default configuration (port 8080)
    HttpCanServer::new()
        .run()
        .await?;

    Ok(())
}
```

### Custom Configuration

```rust
use httpcan::{HttpCanServer, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Method 1: Using builder pattern
    HttpCanServer::new()
        .port(3000)
        .host("127.0.0.1")
        .exclude_header("x-internal-header")
        .run()
        .await?;

    // Method 2: Using ServerConfig
    let config = ServerConfig::new()
        .port(3000)
        .host("127.0.0.1")
        .exclude_headers(vec!["x-internal-header".to_string()]);

    HttpCanServer::with_config(config)
        .run()
        .await?;

    Ok(())
}
```

## Configuration Options

- **port**: Server listening port (default: 8080)
- **host**: Server bind address (default: "0.0.0.0")
- **add_current_server**: Include current server in OpenAPI spec (default: true)
- **exclude_headers**: List of headers to exclude from responses
- **static_dir**: Custom static files directory

## Examples

```bash
# Run basic example
cargo run --example basic

# Run HTTP client testing example (requires examples feature)
# First terminal - start server:
cargo run --example basic

# Second terminal - run client tests:
cargo run --example client_testing --features examples
```

### HTTP Client Testing

The `client_testing` example demonstrates how to use HTTPCan for HTTP client testing, similar to go-httpbin patterns:

```rust
// Test timeout behavior
let client = HttpClient::new("http://127.0.0.1:8080".to_string(), Duration::from_secs(1));
match client.get("/delay/10").await {
    Err(e) if e.to_string().contains("timeout") => {
        println!("✅ Test passed: Got expected timeout error");
    }
    _ => println!("❌ Test failed: Expected timeout but request succeeded"),
}
```

Test scenarios include:
1. Timeout behavior testing (`/delay/10` endpoint)
2. Successful request testing (`/delay/1` endpoint)
3. JSON response validation (`/json` endpoint)
4. Status code handling (`/status/404` endpoint)
5. Header echoing verification (`/headers` endpoint)
6. Query parameter handling (`/get` endpoint)

### Unit Tests

The project includes corresponding unit tests (`tests/client_testing_tests.rs`) demonstrating these patterns in a test framework:

```bash
# Start server
cargo run --example basic

# Run tests in another terminal
cargo test --features examples -- --ignored
```

## Binary Usage

You can still use HTTPCan as a standalone binary:

```bash
# Default configuration
cargo run

# Custom port
cargo run -- --port 3000

# View all options
cargo run -- --help
```

## API Compatibility

This library provides httpbin.org-compatible HTTP testing endpoints including HTTP methods, authentication, status codes, redirects, request inspection, response formats, dynamic data, streaming responses, and more.

Complete API documentation is available at the `/openapi.json` endpoint.
