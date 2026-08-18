# HTTPCan

A modern, high‑performance superset of [httpbin.org](https://httpbin.org) for testing HTTP clients, proxies, and AI agents — built with Rust and Actix Web.

[![Crates.io](https://img.shields.io/crates/v/httpcan.svg)](https://crates.io/crates/httpcan)
[![ghcr.io](https://img.shields.io/badge/ghcr.io-seedvector%2Fhttpcan-1f6feb?logo=github)](https://github.com/orgs/seedvector/packages/container/package/httpcan)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Quick Links: [Quick Start](#quick-start) · [Installation](#installation) · [Configuration](#-configuration) · [Examples](#usage-examples) · [OpenAPI & Homepage](#openapi--homepage) · [API Reference](#api-reference) · [Production](#-production-considerations) · [Library](#-library-usage) · [Development](#development) · [License](#license)

## ✨ Features

- **80-endpoint superset of httpbin.org**: every httpbin.org endpoint covered and drop‑in compatible, plus 29 endpoints httpbin.org doesn't have and 18 it has but httpcan fixes or extends — see the [homepage](#openapi--homepage) for the full badge‑tagged list
- **Anti‑phishing redirects**: browser clients hitting `/redirect-to` see a confirmation page instead of a silent 302, closing an open‑redirect abuse vector
- **AI‑friendly streaming**: native `/sse` and `/ndjson` endpoints with OpenAI/Ollama‑compatible chunk formats
- **LLM API mock**: OpenAI- and Anthropic-compatible `/llm` endpoints — point any SDK `base_url` at it (streaming included) and test AI clients without a paid provider
- **Cloud‑native observability**: `/healthz` liveness probe, `/tags` instance identification, and `Server-Timing`/`X-Httpcan-Version` headers on every response
- **Correct header handling**: duplicate and non‑ASCII request headers are preserved instead of being dropped or crashing the server
- **Safer by default**: built‑in filtering strips ~100 reverse‑proxy/CDN headers from echoed responses, with `--exclude-headers` for more
- **Self‑documenting homepage**: `/` lists every endpoint by category with compatibility badges and one‑click "Copy curl" buttons — fully static HTML, crawlable by search engines and AI agents
- **Tiny & fast**: <10MB Docker image, minimal memory footprint, high throughput via Actix Web + Tokio

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
git clone https://github.com/seedvector/httpcan.git
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
| `-p, --port <PORT>`            | Port number to listen on. Also settable via `HTTPCAN_PORT`                                                        | `8080`  | `--port 3000`                                             |
| `--openapi-servers <MODE>`     | How `/openapi.json` builds its `servers` array: `current-first` (default) prepends the instance the visitor is talking to; `spec-only` serves the spec's own servers verbatim. Also settable via `HTTPCAN_OPENAPI_SERVERS` | `current-first` | `--openapi-servers spec-only` |
| `--exclude-headers <HEADERS>`  | Exclude headers in responses; comma‑separated; supports wildcard suffix (e.g., `x-bar-*`). Also settable via `HTTPCAN_EXCLUDE_HEADERS` | `""`    | `--exclude-headers "x-forwarded-*,cf-*,server"`           |
| `--max-bytes <BYTES>`          | Max bytes for `/bytes`, `/stream-bytes`, `/range`, and `/drip`; over‑limit returns 404 instead of truncating (httpbin #594). Also settable via `HTTPCAN_MAX_BYTES` | `102400` | `--max-bytes 1048576`                                     |
| `--canonical-scheme <auto\|http\|https>` | Scheme for SEO‑facing URLs only (canonical link, sitemap.xml, robots.txt); doesn't affect copy‑curl examples or the OpenAPI current server, which always mirror the actual request. Also settable via `HTTPCAN_CANONICAL_SCHEME` | `auto`  | `--canonical-scheme https` |
| `--static-dir <DIR>`          | Static assets directory: user overrides for `openapi.json`, `favicon.png`, `index.html`, `robots.txt`, `sitemap.xml`, plus extra files served at `/static/<name>` or `/<name>`. Also settable via `HTTPCAN_STATIC_DIR` | *binary-relative `static`, else `./static`* | `--static-dir /etc/httpcan/static` |
| `-h, --help`                   | Print help information                                                                                             |         | `--help`                                                  |
| `-V, --version`                | Print version                                                                                                      |         | `--version`                                               |

Notes:
- Built‑in filtering includes reverse proxy/CDN providers (Nginx, Cloudflare, AWS, GCP, Azure).
- When using Docker, ensure `-p host:container` mapping matches your `--port` if you override it.

Any flag backed by an environment variable (e.g. `HTTPCAN_CANONICAL_SCHEME`) can also be set via a `.env` file in the working directory — handy for a bare‑binary deployment on a server, where there's no Docker `--env-file`/`env_file:` or systemd `EnvironmentFile=` to inject real environment variables for you:

```bash
# .env
HTTPCAN_CANONICAL_SCHEME=https
```

```bash
./httpcan   # picks up .env from the current directory automatically
```

`.env` values are loaded before CLI flags are parsed, so an explicit `--canonical-scheme` flag still takes precedence over `HTTPCAN_CANONICAL_SCHEME` in `.env`.

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

# Inject response headers (rate-limit / Retry-After testing)
curl -H "Accept: application/json" "http://localhost:8080/status/429?header=Retry-After:120&header=X-RateLimit-Remaining:0"

# Redirect to a URL (supports form/json)
curl -X POST http://localhost:8080/redirect-to -d "url=https://example.com"
```

> **🔒 Open‑redirect protection on `/redirect‑to`**
>
> Unlike a naive 302, `/redirect-to` detects **browser clients** (via `Accept: text/html`) and returns an **interstitial warning page** instead of silently redirecting. This prevents phishing abuse where attackers exploit your trusted domain (e.g. `…/redirect-to?url=https://evil.com`).
>
> - Programmatic clients (curl, httpx, …) still receive the standard `302` — no API breakage.
> - Browser clients get a `200` HTML page showing the destination URL and requiring an explicit click.
> - Non‑`http(s)` URL schemes (`javascript:`, `data:`, …) render as a **disabled link**.
> - Destination URLs are HTML‑escaped in the interstitial to prevent XSS.
> - The page carries `X-Robots-Tag: noindex` to avoid search‑engine indexing.

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

### LLM API Mock

```bash
# OpenAI-compatible chat completions (streaming supported)
curl -X POST http://localhost:8080/llm/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.6","messages":[{"role":"user","content":"Hello"}]}'

# Anthropic-compatible messages
curl -X POST http://localhost:8080/llm/v1/messages \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-sonnet-5","max_tokens":64,"messages":[{"role":"user","content":"Hello"}]}'

# OpenAI-compatible Responses API
curl -X POST http://localhost:8080/llm/v1/responses \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.6","input":"Hello"}'

# OpenAI-compatible legacy completions (text-in/text-out)
curl -X POST http://localhost:8080/llm/v1/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.6","prompt":"Once upon a time"}'
```

Point an SDK at it — no API key needed (any value is accepted), and responses are deterministic:

```python
from openai import OpenAI

client = OpenAI(base_url="http://localhost:8080/llm/v1", api_key="not-needed")
reply = client.chat.completions.create(
    model="gpt-5.6",
    messages=[{"role": "user", "content": "Hello"}],
    extra_body={"httpcan": {"content": "Custom reply text"}},
)
print(reply.choices[0].message.content)  # Custom reply text
```

The Anthropic SDK works with `base_url="http://localhost:8080/llm"` (it appends `/v1/messages` itself). `GET /llm` returns a self-describing index, and `GET /llm/v1/models` returns the model list in whichever family's shape the request looks like (an `anthropic-version` header switches it).

### Cookies & Inspection

```bash
curl http://localhost:8080/cookies
curl http://localhost:8080/headers
curl http://localhost:8080/ip
```

## OpenAPI & Homepage

- OpenAPI spec: `GET /openapi.json`
- API catalog (RFC 9727, automated discovery): `GET /.well-known/api-catalog` — a Linkset pointing agents at the spec, docs, and health probe; URLs mirror the request origin
- Homepage: visit `/` for a static, crawlable page listing every endpoint by category, each with a compatibility badge (`Enhanced`/`New`) relative to httpbin.org and a one‑click "Copy curl" button (pre‑filled with sample parameters and resolved against the instance you're viewing). It always renders as HTML, regardless of the `Accept` header — no JavaScript required to read the content.

## API Reference

Endpoints are grouped into the same categories shown on the homepage (`/`) — visit a running instance for the full interactive list with compatibility badges and copy‑ready curl examples:

| Category | Endpoints |
|---|---|
| HTTP Methods | `/get` `/post` `/put` `/patch` `/delete` `/method` `/head` `/options` `/trace` `/query` `/echo` |
| Anything | `/anything` `/anything/{path}` |
| Auth | `/basic-auth` `/hidden-basic-auth` `/bearer` `/jwt-bearer` `/digest-auth` |
| Status codes | `/status/{codes}` |
| Request inspection | `/headers` `/ip` `/user-agent` |
| Response inspection | `/cache` `/etag` `/response-headers` |
| Response formats | `/json` `/xml` `/html` `/robots.txt` `/deny` `/encoding/utf8` `/encoding/iso-8859-1` `/gzip` `/deflate` `/brotli` `/zstd` |
| Dynamic data | `/uuid` `/base64` `/bytes` `/stream-bytes` `/stream` `/range` `/links` `/drip` `/delay` |
| Cookies | `/cookies` `/cookies/set` `/cookies/delete` |
| Images | `/image` `/image/png` `/image/jpeg` `/image/webp` `/image/svg` |
| Redirects | `/redirect` `/relative-redirect` `/absolute-redirect` `/redirect-to` |
| Streaming | `/sse` `/ndjson` |
| AI mock | `/llm/v1/chat/completions` `/llm/v1/messages` `/llm/v1/responses` `/llm/v1/completions` `/llm/v1/models` |
| Observability | `/healthz` `/tags` |

Every endpoint carries a compatibility badge relative to httpbin.org:

- *(no badge)* — drop‑in compatible with httpbin.org
- **Enhanced** — httpbin.org has this endpoint, but httpcan fixes a bug or extends it
- **New** — not available in httpbin.org

For full parameter details and schemas, consult the [OpenAPI spec](/openapi.json).

### HTTPCan Enhancements

- Echo endpoint: `/echo` reflects request body and headers (multi‑method)
- Methods+: `QUERY` HTTP method (RFC 9430 — a safe, idempotent GET with a body) accepted on `/anything`, `/anything/{anything}`, and `/echo`, with a dedicated `/query` endpoint echoing URL args plus the parsed body
- Auth+: Basic auth with username only; JWT Bearer decode/inspect at `/jwt-bearer`
- Status+: Content‑type priority: `Accept` > request `Content-Type` > default; supports custom bodies via query/body
- Redirects+: `POST /redirect-to` supports `application/x-www-form-urlencoded`, `multipart/form-data`, `application/json`; browser clients get an open-redirect interstitial (see [above](#status--redirects))
- Streaming+: SSE/NDJSON endpoints with `count`, `delay`, and AI formats (OpenAI/Ollama)
- File uploads+: Multiple files with the same field return as array across multipart endpoints
- Observability+: `/healthz` liveness probe; `/tags` exposes `HTTPCAN_*` env vars; every response carries `Server-Timing` and `X-Httpcan-Version` headers
- Method echo+: `/method` echoes any method name; `/head` `/options` `/trace` `/query` are dedicated echo endpoints; GET endpoints also answer `HEAD`
- Status headers+: `?header=Name:Value` injects response headers on `/status/{codes}` (repeatable; e.g. `Retry-After` for rate‑limit testing)
- Body encoding+: POST to `/gzip`, `/deflate`, `/brotli`, `/zstd`, or `/base64` returns the request body in the matching encoding

## 🚀 Production Considerations

Before deploying HTTPCan on the public internet, review these hardening options:

- **Open‑redirect protection on `/redirect‑to`**: Browser clients (detected via `Accept: text/html`) receive an interstitial warning page showing the destination URL instead of a silent `302`. This blocks phishing abuse via `…/redirect-to?url=https://evil.com` on your trusted domain. Programmatic clients (curl, httpx, …) still get the standard `302` for API compatibility. Non‑`http(s)` URL schemes render as a disabled link.
- **No reflected XSS on `/base64`**: Decoded content is always returned as `text/plain; charset=utf-8`, never `text/html`. Browsers display the raw text without rendering embedded `<script>` tags.
- **Response header filtering**:
  - **Built‑in (always on)**: ~100 reverse‑proxy/CDN headers are stripped from all echoed responses by default — Nginx (`x-real-ip`, `x-forwarded-*`), Cloudflare (`cf-*`), AWS CloudFront/ALB (`cloudfront-*`, `x-amzn-*`), GCP (`x-appengine-*`, `x-cloud-trace-context`), Azure (`x-azure-*`, `x-ms-*`). Prevents infrastructure information leakage without any configuration.
  - **Custom (`--exclude-headers`)**: Add your own patterns to strip additional sensitive headers, with wildcard suffix support: `--exclude-headers "x-internal-*,server,x-secret-token"`.
- **Resource limits**: `--max-bytes` caps `/bytes` and `/stream-bytes` responses (default 100KB); over‑limit requests return `404` instead of silently truncating (httpbin #594).
- **Non‑root Docker**: The official image runs as a dedicated unprivileged user (`uid 10001`).

## 🎨 Self-Hosting & Customization

The binary is fully self-contained: the OpenAPI spec and favicon are embedded at compile time, and `/`, `/robots.txt`, and `/sitemap.xml` are generated per request. To customize your instance, drop files into a static assets directory (`--static-dir`, or the default `static/` next to the binary / `./static`; in Docker, mount a volume at `/httpcan/static` — files must be world-readable for `uid 10001`):

| File in static dir | Replaces | Notes |
|---|---|---|
| `openapi.json` | embedded spec at `/openapi.json` | The current server is still injected into `servers` (verbatim with `--openapi-servers spec-only`) |
| `favicon.png` | embedded favicon at `/favicon.png` | |
| `index.html` | generated homepage at `/` | Forfeits origin-resolved curl examples, markdown negotiation, and Link headers |
| `robots.txt` | generated policy at `/robots.txt` | E.g. `Disallow: *` for private instances |
| `sitemap.xml` | generated index at `/sitemap.xml` | Absolute URLs in a static file don't follow the request origin or `--canonical-scheme` |

Behavior notes:

- Files are checked per request — adding or replacing a file takes effect immediately, no restart. The directory itself must exist at startup.
- Every other file in the directory is served at `/static/<name>` (and at `/<name>` when the name doesn't collide with an API route).
- Reserved: files named after API routes (`json`, `get`, `image`, …) never shadow those routes; dot‑prefixed files are rejected with 400.

## 🦀 Library Usage

Add dependency:

```toml
[dependencies]
httpcan = "0.6"
```

Embed server:

```rust
use httpcan::HttpCanServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    HttpCanServer::new()
        .port(3000)
        .host("127.0.0.1")
        .exclude_headers(vec!["foo".into(), "x-bar-*".into()])
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