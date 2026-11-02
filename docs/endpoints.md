# Endpoint reference

Endpoints are grouped into the same categories as the homepage (`/`). Open a running instance, for example <https://httpcan.org>, for the full list with compatibility badges and ready-to-copy curl examples. For parameters and schemas, see the OpenAPI spec at [`/openapi.json`](https://httpcan.org/openapi.json).

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
| AI API Mock | `/llm/v1/chat/completions` `/llm/v1/messages` `/llm/v1/responses` `/llm/v1/completions` `/llm/v1/models`, see [llm-mock.md](llm-mock.md) |
| OAuth 2.0 Mock | `/oauth2/authorize` `/oauth2/token` `/oauth2/userinfo`, see [oauth2.md](oauth2.md) |
| Observability | `/healthz` `/tags` |

Every endpoint carries a compatibility badge relative to httpbin.org:

- *(no badge)*: drop-in compatible with httpbin.org
- **Enhanced**: httpbin.org has this endpoint, but httpcan fixes a bug or extends it
- **New**: not available in httpbin.org

## Examples

```bash
# Basic GET and JSON POST
curl https://httpcan.org/get
curl -X POST https://httpcan.org/post -H "Content-Type: application/json" -d '{"key":"value"}'

# Basic auth, including username-only (empty password)
curl -u username:password https://httpcan.org/basic-auth/username/password
curl -u username: https://httpcan.org/basic-auth/username

# Status codes: a specific code, a random pick from a list, injected headers
curl https://httpcan.org/status/418
curl https://httpcan.org/status/200,404,500
curl -H "Accept: application/json" "https://httpcan.org/status/429?header=Retry-After:120&header=X-RateLimit-Remaining:0"

# Redirect (form, multipart or JSON body)
curl -X POST https://httpcan.org/redirect-to -d "url=https://example.com"

# Compression and formats
curl -H "Accept-Encoding: gzip" https://httpcan.org/gzip
curl https://httpcan.org/json
curl https://httpcan.org/xml

# Streaming in OpenAI / Ollama chunk formats
curl "https://httpcan.org/sse?count=3&format=simple"
curl "https://httpcan.org/sse/5?format=openai&delay=2000"
curl "https://httpcan.org/ndjson?count=3&format=simple"
curl "https://httpcan.org/ndjson/5?format=ollama&model=llama3&delay=1500"

# Inspection
curl https://httpcan.org/cookies
curl https://httpcan.org/headers
curl https://httpcan.org/ip
```

## What httpcan adds or fixes compared with httpbin.org

- **Echo**: `/echo` reflects the request body and headers (any method).
- **Methods**: the HTTP `QUERY` method (a safe, idempotent request with a body) is accepted on `/anything`, `/anything/{anything}` and `/echo`, and the dedicated `/query` endpoint echoes URL args plus the parsed body. `/method` echoes any method name; `/head`, `/options`, `/trace` and `/query` are dedicated echo endpoints; GET endpoints also answer `HEAD`.
- **Auth**: Basic auth with username only; JWT decoding and inspection at `/jwt-bearer`.
- **Status**: the response content type follows `Accept`, then the request's `Content-Type`, then a default. Custom bodies via query or body. `?header=Name:Value` injects response headers (repeatable, e.g. `Retry-After` for rate-limit testing).
- **Redirects**: `POST /redirect-to` accepts `application/x-www-form-urlencoded`, `multipart/form-data` and `application/json`. Browser clients get an open-redirect warning page (below).
- **Streaming**: SSE and NDJSON endpoints with `count`, `delay`, and OpenAI/Ollama formats.
- **File uploads**: multiple files with the same field name come back as an array on every multipart endpoint.
- **Body encoding**: POST to `/gzip`, `/deflate`, `/brotli`, `/zstd` or `/base64` returns the request body in that encoding.
- **Headers**: duplicate and non-ASCII request headers are kept instead of being dropped or crashing the server.
- **Observability**: `/healthz` liveness probe; `/tags` exposes `HTTPCAN_*` environment variables; every response carries `Server-Timing` and `X-Httpcan-Version` headers.

### Open-redirect protection on `/redirect-to`

A plain 302 would let anyone use your domain for phishing (`…/redirect-to?url=https://evil.com`). Instead, `/redirect-to` checks for **browser clients** (via `Accept: text/html`) and shows them a **warning page** rather than redirecting silently.

- Programmatic clients (curl, httpx, …) still get the standard `302`, so nothing breaks for API callers.
- Browser clients get a `200` HTML page that shows the destination URL and needs an explicit click.
- Non-`http(s)` URL schemes (`javascript:`, `data:`, …) render as a disabled link.
- Destination URLs are HTML-escaped on the warning page to prevent XSS.
- The page carries `X-Robots-Tag: noindex` so search engines skip it.

## Discovery

- OpenAPI spec: `GET /openapi.json`
- API catalog (RFC 9727): `GET /.well-known/api-catalog` returns a Linkset pointing at the spec, docs and health probe; URLs follow the request origin.
- Homepage: `/` lists every endpoint by category with its compatibility badge and a one-click "Copy curl" button that targets the instance you're viewing. It is static HTML with no JavaScript needed to read it.
