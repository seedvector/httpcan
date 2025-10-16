# HTTPCan

A simple, high‑performance HTTP request & response service built with Rust and Actix Web. Fully compatible with [httpbin.org](https://httpbin.org), with modern streaming and AI‑friendly enhancements.

[![Crates.io](https://img.shields.io/crates/v/httpcan.svg)](https://crates.io/crates/httpcan)
[![ghcr.io](https://img.shields.io/badge/ghcr.io-seedvector%2Fhttpcan-1f6feb?logo=github)](https://github.com/orgs/seedvector/packages/container/package/httpcan)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Quick Links: [Quick Start](#quick-start) · [Installation](#installation) · [Configuration](#-configuration) · [Examples](#usage-examples) · [OpenAPI & Web UI](#openapi--web-ui) · [API Reference](#api-reference) · [Library](#-library-usage) · [Development](#development) · [License](#license)

## ✨ Features

- **HTTPBin compatible**: Use as a drop‑in replacement for testing/migration
- **Modern streaming**: Native SSE and NDJSON, AI‑compatible formats (OpenAI/Ollama)
- **Tiny Docker image**: <10MB, fast to pull and start
- **Minimal memory footprint**: Efficient async Rust I/O
- **High throughput**: Actix Web + Tokio

## Quick Start

Choose one way to run:

```bash
# Docker (recommended)
docker run -p 8080:8080 ghcr.io/seedvector/httpcan:latest
curl http://localhost:8080/get

# Cargo
cargo install httpcan
httpcan
curl http://localhost:8080/get
```

## Installation

### 🐳 Docker

```bash
# Latest image
docker run -p 8080:8080 ghcr.io/seedvector/httpcan:latest

# Custom port
docker run -p 3000:3000 ghcr.io/seedvector/httpcan:latest --port 3000

# Header filtering
docker run -p 8080:8080 ghcr.io/seedvector/httpcan:latest --exclude-headers "foo, x-bar-*"
```

### 📦 Cargo

```bash
# Install globally
cargo install httpcan

# Run
httpcan
httpcan --port 3000
httpcan --exclude-headers "foo, x-bar-*"
httpcan --port 3000 --no-current-server --exclude-headers "x-forwarded-*,cf-*"
```

### 🛠️ From Source

```bash
git clone https://github.com/<your-org-or-user>/httpcan.git
cd httpcan

# Default (8080)
cargo run

# Custom port
cargo run -- --port 3000

# Release build
cargo build --release
./target/release/httpcan --port 8080
```

## 🧰 Configuration

CLI flags:

| Option                         | Description                                                                                                       | Default | Example                                                    |
|--------------------------------|-------------------------------------------------------------------------------------------------------------------|---------|------------------------------------------------------------|
| `-p, --port <PORT>`            | Port number to listen on                                                                                          | `8080`  | `--port 3000`                                             |
| `--no-current-server`          | Do not add current server to OpenAPI `servers` list                                                               | `false` | `--no-current-server`                                     |
| `--exclude-headers <HEADERS>`  | Exclude headers in responses; comma‑separated; supports wildcard suffix (e.g. `x-bar-*`)                          | `""`    | `--exclude-headers "x-forwarded-*,cf-*,server"`           |
| `-h, --help`                   | Print help information                                                                                             |         | `--help`                                                  |
| `-V, --version`                | Print version                                                                                                      |         | `--version`                                               |

Notes:
- Built‑in filtering includes reverse proxy/CDN providers (Nginx, Cloudflare, AWS, GCP, Azure).
- When using Docker, ensure `-p host:container` mapping matches your `--port` if you override it.

## Usage Examples

```bash
# Basic GET
curl http://localhost:8080/get

# POST with JSON
curl -X POST http://localhost:8080/post \
  -H "Content-Type: application/json" \
  -d '{"key":"value"}'
```

### Auth

```bash
# Basic auth
curl -u username:password http://localhost:8080/basic-auth/username/password

# Username only (empty password) — enhanced
curl -u username: http://localhost:8080/basic-auth/username
```

### Status & Redirects

```bash
# Specific status
curl http://localhost:8080/status/418

# Random from list
curl http://localhost:8080/status/200,404,500

# Redirect to a URL (supports form/json)
curl -X POST http://localhost:8080/redirect-to -d "url=https://example.com"
```

### Compression & Formats

```bash
curl -H "Accept-Encoding: gzip" http://localhost:8080/gzip
curl http://localhost:8080/json
curl http://localhost:8080/xml
```

### Streaming (SSE/NDJSON)

```bash
# SSE
curl http://localhost:8080/sse?count=3&format=simple
curl http://localhost:8080/sse/5?format=openai&delay=2000

# NDJSON
curl http://localhost:8080/ndjson?count=3&format=simple
curl http://localhost:8080/ndjson/5?format=ollama&model=llama3&delay=1500
```

### Cookies & Inspection

```bash
curl http://localhost:8080/cookies
curl http://localhost:8080/headers
curl http://localhost:8080/ip
```

## OpenAPI & Web UI

- OpenAPI spec: `GET /openapi.json`
- Web UI / API info: visit `/` in a browser; renders HTML or JSON based on `Accept` header

## API Reference

### HTTPBin Compatibility (Overview)

- Methods: `GET /get`, `POST /post`, `PUT /put`, `PATCH /patch`, `DELETE /delete`
- Anything: `/anything`, `/anything/{anything}` (supports multiple methods)
- Auth: Basic, Hidden Basic, Digest
- Formats: JSON, XML, HTML, `robots.txt`, `encoding/utf8`, gzip/deflate/brotli
- Dynamic: `uuid`, `bytes`, `stream`, `range`, `links`, `delay`, `drip`
- Redirects: `redirect`, `relative-redirect`, `absolute-redirect`, `redirect-to`
- Inspection: `headers`, `ip`, `user-agent`
- Response: `cache`, `etag`, `response-headers`
- Cookies: `cookies` CRUD
- Images: `image`, `image/png`, `image/jpeg`, `image/webp`, `image/svg`
- Status: `/status/{codes}` (single or comma‑separated)

For the full, up‑to‑date list and schemas, consult the [OpenAPI spec](/openapi.json).

### HTTPCan Enhancements

- Echo endpoint: `/echo` reflects request body and headers (multi‑method)
- Auth+: Basic auth with username only; JWT Bearer decode/inspect at `/jwt-bearer`
- Status+: Content‑type priority: `Accept` > request `Content-Type` > default; supports custom bodies via query/body
- Redirects+: `POST /redirect-to` supports `application/x-www-form-urlencoded`, `multipart/form-data`, `application/json`
- Streaming+: SSE/NDJSON endpoints with `count`, `delay`, and AI formats (OpenAI/Ollama)
- File uploads+: Multiple files with the same field return as array across multipart endpoints

## 🦀 Library Usage

Add dependency:

```toml
[dependencies]
httpcan = "0.5"
```

Embed server:

```rust
use httpcan::HttpCanServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    HttpCanServer::new()
        .port(3000)
        .host("127.0.0.1")
        .exclude_header("foo, x-bar-*")
        .run()
        .await?;
    Ok(())
}
```

More examples and advanced config: see [LIBRARY_USAGE.md](LIBRARY_USAGE.md).

## Development

```bash
# Run checks
cargo fmt --all
cargo clippy --all -- -D warnings
cargo test

# Run locally
cargo run -- --port 8080
```

Contributions are welcome! Please open issues/PRs for discussion.

## License

MIT — see [LICENSE](LICENSE).