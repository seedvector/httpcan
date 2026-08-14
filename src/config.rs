use clap::{Parser, ValueEnum};

/// Default maximum number of bytes returned by `/bytes` and `/stream-bytes`.
/// Matches httpbin's 100KB cap (httpbin #594).
pub const DEFAULT_MAX_BYTES: usize = 100 * 1024;

/// Controls the scheme (`http`/`https`) used for the site's SEO-facing
/// self-identification surfaces: the homepage `<link rel="canonical">`,
/// `sitemap.xml`, and the `Sitemap:` directive in `robots.txt`.
///
/// This intentionally does *not* affect user-facing URLs such as the
/// copy-curl examples on the homepage or the OpenAPI "current server"
/// entry — those keep mirroring the scheme/host the visitor actually used,
/// so they stay directly runnable (e.g. a plain-http self-hosted instance
/// still shows working `http://` examples).
///
/// `Auto` trusts the incoming request (via `ConnectionInfo`, which honors
/// `X-Forwarded-Proto`/`Forwarded`). That is correct for most self-hosted
/// deployments, but silently resolves to `http` when a TLS-terminating
/// reverse proxy or CDN sits in front and doesn't forward the original
/// scheme — which then tells search engines that the insecure URL is the
/// canonical one. Pin `Http`/`Https` to bypass request detection entirely
/// for these SEO surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
#[value(rename_all = "lower")]
pub enum SchemeOverride {
    #[default]
    Auto,
    Http,
    Https,
}

impl SchemeOverride {
    /// The fixed scheme to use, or `None` if it should be auto-detected
    /// from the request.
    pub fn fixed(self) -> Option<&'static str> {
        match self {
            SchemeOverride::Auto => None,
            SchemeOverride::Http => Some("http"),
            SchemeOverride::Https => Some("https"),
        }
    }
}

/// HTTPCan - HTTP testing service similar to httpbin.org
#[derive(Parser)]
#[command(name = "httpcan")]
#[command(about = "A simple HTTP request & response service", long_about = None)]
#[command(version)]
pub struct Args {
    /// Port number to listen on
    #[arg(short, long, env = "HTTPCAN_PORT", default_value_t = 8080)]
    pub port: u16,

    /// How `/openapi.json` builds its `servers` array: `current-first` (default) prepends the instance the visitor is talking to, so imported tools (Swagger UI, Postman) work out of the box; `spec-only` serves the spec's own servers verbatim.
    #[arg(
        long,
        value_enum,
        env = "HTTPCAN_OPENAPI_SERVERS",
        default_value = "current-first"
    )]
    pub openapi_servers: OpenapiServers,

    /// Exclude specific headers from responses. Comma-separated list of header keys, supports wildcard suffix matching (e.g., "foo, x-bar-*")
    #[arg(long, env = "HTTPCAN_EXCLUDE_HEADERS")]
    pub exclude_headers: Option<String>,

    /// Maximum bytes returned by `/bytes/{n}`, `/stream-bytes/{n}`, `/range/{n}`, and `/drip` (`numbytes`). Requests exceeding this return a 404 instead of silently truncating.
    #[arg(long, env = "HTTPCAN_MAX_BYTES", default_value_t = DEFAULT_MAX_BYTES)]
    pub max_bytes: usize,

    /// SEO-facing URLs only — scheme for the canonical link, sitemap.xml, and the robots.txt Sitemap directive. Does not affect copy-curl examples or the OpenAPI current server, which always mirror the visitor's actual request. "auto" detects it from the request, which can be wrong behind a reverse proxy/CDN that doesn't forward X-Forwarded-Proto; pin http/https explicitly in that case.
    #[arg(
        long,
        value_enum,
        env = "HTTPCAN_CANONICAL_SCHEME",
        default_value = "auto"
    )]
    pub canonical_scheme: SchemeOverride,

    /// Directory for user-overridable assets (openapi.json, favicon.png, index.html, robots.txt, sitemap.xml) and extra files served at `/static/<name>` or `/<name>`. A file here with one of those five names replaces the built-in default at its canonical URL. Default: the `static` directory next to the binary, falling back to `./static`.
    #[arg(long, value_name = "DIR", env = "HTTPCAN_STATIC_DIR")]
    pub static_dir: Option<String>,
}

/// How `/openapi.json` builds its `servers` array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OpenapiServers {
    /// Prepend the current server (mirroring the visitor's request) before
    /// the spec's own servers.
    #[default]
    CurrentFirst,
    /// Serve the spec's own `servers` array verbatim, no injection.
    SpecOnly,
}

impl OpenapiServers {
    /// Whether the OpenAPI handler should inject the current server.
    pub fn add_current_server(self) -> bool {
        matches!(self, OpenapiServers::CurrentFirst)
    }
}
