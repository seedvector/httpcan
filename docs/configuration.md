# Configuration, self-hosting and production

## CLI flags

| Option | Description | Default | Example |
|---|---|---|---|
| `-p, --port <PORT>` | Port number to listen on. Also settable via `HTTPCAN_PORT` | `8080` | `--port 3000` |
| `--openapi-servers <MODE>` | How `/openapi.json` builds its `servers` array: `current-first` (default) puts the instance the visitor is talking to first; `spec-only` serves the spec's own servers unchanged. Also settable via `HTTPCAN_OPENAPI_SERVERS` | `current-first` | `--openapi-servers spec-only` |
| `--exclude-headers <HEADERS>` | Exclude headers in responses; comma-separated; supports wildcard suffix (e.g., `x-bar-*`). Also settable via `HTTPCAN_EXCLUDE_HEADERS` | `""` | `--exclude-headers "x-forwarded-*,cf-*,server"` |
| `--max-bytes <BYTES>` | Max bytes for `/bytes`, `/stream-bytes`, `/range`, and `/drip`; over-limit returns an error instead of truncating (httpbin #594). Also settable via `HTTPCAN_MAX_BYTES` | `102400` | `--max-bytes 1048576` |
| `--canonical-scheme <auto\|http\|https>` | Scheme for SEO-facing URLs only (canonical link, sitemap.xml, robots.txt); doesn't affect copy-curl examples or the OpenAPI current server, which always mirror the actual request. Also settable via `HTTPCAN_CANONICAL_SCHEME` | `auto` | `--canonical-scheme https` |
| `--static-dir <DIR>` | Static assets directory: user overrides for `openapi.json`, `favicon.png`, `index.html`, `robots.txt`, `sitemap.xml`, plus extra files served at `/static/<name>` or `/<name>`. Also settable via `HTTPCAN_STATIC_DIR` | *`static` next to the binary, else `./static`* | `--static-dir /etc/httpcan/static` |
| `--oauth2-clients <ID:SECRET,…>` | OAuth 2.0 client registry for `/oauth2`: when set, client secrets are validated (`invalid_client` on mismatch); when empty, any non-empty secret passes (mock mode). Also settable via `HTTPCAN_OAUTH2_CLIENTS` | *(empty)* | `--oauth2-clients "webapp:s3cr3t,cli:an0ther"` |
| `--llm-models <ID,…>` | Model ids listed by `/llm/v1/models` for both API families, replacing the default `httpcan-openai` / `httpcan-anthropic`. `httpcan-auto-tools` is always appended. Requests accept any model id either way; this only changes what model pickers show. Also settable via `HTTPCAN_LLM_MODELS` | *(empty)* | `--llm-models "gpt-5.5,claude-opus-5-5"` |
| `-h, --help` | Print help information | | |
| `-V, --version` | Print version | | |

With Docker, make sure the `-p host:container` mapping matches your `--port` if you change it:

```bash
docker run -p 3000:3000 ghcr.io/seedvector/httpcan:latest --port 3000
```

### `.env` files

Any flag backed by an environment variable can also be set in a `.env` file in the working directory. That's useful for a bare-binary deployment, where there's no Docker `--env-file` or systemd `EnvironmentFile=` to set real environment variables:

```bash
# .env
HTTPCAN_CANONICAL_SCHEME=https
```

```bash
./httpcan   # picks up .env from the current directory automatically
```

`.env` values are loaded before CLI flags are parsed, so an explicit `--canonical-scheme` flag still overrides `HTTPCAN_CANONICAL_SCHEME` from `.env`.

## Production considerations

Before exposing an instance on the public internet:

- **Open-redirect protection on `/redirect-to`**: browser clients (detected via `Accept: text/html`) get a warning page showing the destination URL instead of a silent `302`. This stops phishers using `…/redirect-to?url=https://evil.com` on your trusted domain. Programmatic clients (curl, httpx, …) still get the standard `302`. Non-`http(s)` URL schemes render as a disabled link, destination URLs are HTML-escaped, and the page carries `X-Robots-Tag: noindex`.
- **No reflected XSS on `/base64`**: decoded content is always returned as `text/plain; charset=utf-8`, never `text/html`.
- **Response header filtering**:
  - *Built-in (always on)*: about 100 reverse-proxy/CDN headers are stripped from echoed responses: Nginx (`x-real-ip`, `x-forwarded-*`), Cloudflare (`cf-*`), AWS CloudFront/ALB (`cloudfront-*`, `x-amzn-*`), GCP (`x-appengine-*`, `x-cloud-trace-context`), Azure (`x-azure-*`, `x-ms-*`).
  - *Custom*: `--exclude-headers "x-internal-*,server,x-secret-token"` strips more, with wildcard suffix support.
- **Resource limits**: `--max-bytes` caps `/bytes`, `/stream-bytes`, `/range` and `/drip` (default 100 KB). Over-limit requests are rejected rather than silently truncated (httpbin #594).
- **OAuth secrets**: `/oauth2` accepts any client secret unless `--oauth2-clients` is set.
- **Non-root Docker**: the official image runs as an unprivileged user (`uid 10001`).
- **Health and identity**: `/healthz` is a liveness probe; `/tags` exposes `HTTPCAN_*` environment variables; every response carries `Server-Timing` and `X-Httpcan-Version` headers.

## Self-hosting and customization

The binary is fully self-contained: the OpenAPI spec and favicon are embedded at compile time, and `/`, `/robots.txt` and `/sitemap.xml` are generated per request. To customize an instance, drop files into a static assets directory (`--static-dir`, or by default `static/` next to the binary, then `./static`). In Docker, mount a volume at `/httpcan/static`; files must be world-readable for `uid 10001`.

| File in static dir | Replaces | Notes |
|---|---|---|
| `openapi.json` | embedded spec at `/openapi.json` | The current server is still added to `servers` (unless you set `--openapi-servers spec-only`) |
| `favicon.png` | embedded favicon at `/favicon.png` | |
| `index.html` | generated homepage at `/` | You lose curl examples that match the request origin, markdown negotiation, and Link headers |
| `robots.txt` | generated policy at `/robots.txt` | E.g. `Disallow: *` for private instances |
| `sitemap.xml` | generated index at `/sitemap.xml` | Absolute URLs in a static file don't follow the request origin or `--canonical-scheme` |

Behavior notes:

- Files are checked per request, so adding or replacing a file takes effect immediately with no restart. The directory itself must exist at startup.
- Every other file in the directory is served at `/static/<name>` (and at `/<name>` when the name doesn't collide with an API route).
- Files named after API routes (`json`, `get`, `image`, …) never shadow those routes, and dot-prefixed files are rejected with 400.
