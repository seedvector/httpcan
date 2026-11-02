# HTTPCan

An [httpbin.org](https://httpbin.org)-compatible HTTP test server that also mocks the OpenAI, Anthropic and OAuth 2.0 APIs, so you can test HTTP clients, proxies and AI agents without calling real providers.

**Status:** actively developed, pre-1.0 (4 releases since August 2026, see [tags](https://github.com/seedvector/httpcan/tags)). The httpbin-compatible endpoints are stable; the `/llm` mock options and the Rust library API may still change between minor versions. MIT licensed.

[![Crates.io](https://img.shields.io/crates/v/httpcan.svg)](https://crates.io/crates/httpcan)
[![CI](https://github.com/seedvector/httpcan/actions/workflows/ci.yml/badge.svg)](https://github.com/seedvector/httpcan/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## Try it without installing

Every endpoint is live at **<https://httpcan.org>**:

```bash
curl https://httpcan.org/get
```

Point an OpenAI SDK at it. Any API key works, and responses are deterministic:

```python
from openai import OpenAI

client = OpenAI(base_url="https://httpcan.org/llm/v1", api_key="not-needed")
reply = client.chat.completions.create(
    model="gpt-5.6-sol",
    messages=[{"role": "user", "content": "Hello"}],
    extra_body={"httpcan": {"content": "Custom reply text"}},
)
print(reply.choices[0].message.content)  # Custom reply text
```

## Why httpcan

- **Drop-in for httpbin.org.** Every httpbin.org endpoint, same paths and response shapes, plus 33 endpoints httpbin.org doesn't have and 18 it has that httpcan fixes or extends (84 in total). Existing httpbin tests keep working when you change the base URL.
- **LLM API mock.** OpenAI Chat Completions, Responses and legacy completions, plus Anthropic Messages, with streaming. Each request can script its own response: tool calls in the right format for each provider, provider-shaped errors (429, 503/529, 401, …), fixed token usage, and multi-turn tool sequences. The script travels in the request, so the mock is stateless and works through any SDK. GUI chat apps can drive it with `#mock` directives typed into the chat. [Details](docs/llm-mock.md)
- **OAuth 2.0 authorization server.** Authorization code with PKCE, implicit, password, client credentials, rotating refresh tokens, and RFC 8414 discovery. [Details](docs/oauth2.md)
- **One self-contained binary.** No database or external services. The Docker image is about 8 MB and runs as a non-root user.

**When to use something else:**

- You need a WebSocket echo endpoint, random-failure injection (`/unstable`), or a zero-dependency Go library to embed in Go tests: [go-httpbin](https://github.com/mccutchen/go-httpbin) has those, and httpcan doesn't.
- You need real model output or record/replay of real provider traffic: httpcan only returns deterministic placeholders.
- You need to mock *your own* API's endpoints: use a general-purpose mock server such as [WireMock](https://wiremock.org).

## Run your own

```bash
# Docker
docker run -p 8080:8080 ghcr.io/seedvector/httpcan:latest

# or with Cargo (needs a Rust toolchain)
cargo install httpcan
httpcan

# then
curl http://localhost:8080/get
```

The server listens on port 8080 by default. Change it with `--port` (or `HTTPCAN_PORT`). Under Docker, match the `-p host:container` mapping. All flags, `.env` support, header filtering, custom static files and production hardening are in [docs/configuration.md](docs/configuration.md).

From source:

```bash
git clone https://github.com/seedvector/httpcan.git
cd httpcan
cargo run --release -- --port 8080
```

## Examples

```bash
# Status codes: a fixed code, a random pick, injected rate-limit headers
curl https://httpcan.org/status/418
curl https://httpcan.org/status/200,404,500
curl "https://httpcan.org/status/429?header=Retry-After:120"

# Server-sent events in OpenAI chunk format
curl "https://httpcan.org/sse/5?format=openai&delay=2000"

# Anthropic-shaped "overloaded" error from the LLM mock (returns 529)
curl -X POST https://httpcan.org/llm/v1/messages \
  -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"x"}],"httpcan":{"error":"overloaded"}}'
```

## Documentation

| Topic | Where |
|---|---|
| Every endpoint, what changed from httpbin.org, more examples | [docs/endpoints.md](docs/endpoints.md) |
| LLM mock: endpoints, the `httpcan` script block, `#mock` directives, chat apps | [docs/llm-mock.md](docs/llm-mock.md) |
| OAuth 2.0 mock: flows and client validation | [docs/oauth2.md](docs/oauth2.md) |
| CLI flags, self-hosting, production considerations | [docs/configuration.md](docs/configuration.md) |
| Embedding httpcan in Rust tests | [LIBRARY_USAGE.md](LIBRARY_USAGE.md) |
| Machine-readable spec | [OpenAPI](https://httpcan.org/openapi.json) |

Any running instance also serves a homepage at `/` that lists every endpoint with a one-click "Copy curl" button.

## Use as a Rust library

```toml
[dependencies]
httpcan = "0.9"
```

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

## Contributing

Issues and pull requests are welcome at [github.com/seedvector/httpcan](https://github.com/seedvector/httpcan/issues). Before sending a PR:

```bash
cargo fmt --all
cargo clippy --all -- -D warnings
cargo test
```

## License

MIT, see [LICENSE](LICENSE).
