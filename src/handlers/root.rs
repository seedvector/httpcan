use super::*;
use std::fmt::Write as _;

/// Compatibility status relative to the original httpbin.org (see
/// `https://httpbin.org/spec.json`), rendered as a small badge next to each
/// endpoint on the homepage. Endpoints with no badge behave the same as
/// httpbin and are drop-in compatible.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Badge {
    /// httpbin has this endpoint; httpcan fixes a bug or adds a capability.
    Enhanced,
    /// Not available in httpbin.org at all.
    New,
}

impl Badge {
    fn label(self) -> &'static str {
        match self {
            Badge::Enhanced => "Enhanced",
            Badge::New => "New",
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            Badge::Enhanced => "badge badge-enhanced",
            Badge::New => "badge badge-new",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Badge::Enhanced => {
                "httpbin.org has this endpoint; httpcan fixes a bug or adds a capability"
            }
            Badge::New => "Not available in httpbin.org",
        }
    }
}

struct Endpoint {
    methods: &'static str,
    path: &'static str,
    badge: Option<Badge>,
    desc: &'static str,
    /// Example curl invocation, with path/query params already filled in
    /// with realistic sample values. `{base}` is substituted with the
    /// current request's origin (e.g. `http://localhost:8080`) at render
    /// time, so the copied command actually works against this instance.
    curl: &'static str,
}

struct Category {
    id: &'static str,
    title: &'static str,
    desc: &'static str,
    endpoints: &'static [Endpoint],
}

const CATEGORIES: &[Category] = &[
    Category {
        id: "http-methods",
        title: "HTTP Methods",
        desc: "Testing different HTTP verbs.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/get", badge: None, desc: "Echoes the request's query parameters, headers, origin, and URL (GET and HEAD).", curl: "curl '{base}/get?greeting=hello'" },
            Endpoint { methods: "POST", path: "/post", badge: Some(Badge::Enhanced), desc: "Echoes the request's parsed body and uploaded files; same-named multipart fields collect into an array.", curl: "curl -X POST {base}/post -H 'Content-Type: application/json' -d '{\"key\":\"value\"}'" },
            Endpoint { methods: "PUT", path: "/put", badge: Some(Badge::Enhanced), desc: "Same as /post, for PUT requests.", curl: "curl -X PUT {base}/put -H 'Content-Type: application/json' -d '{\"key\":\"value\"}'" },
            Endpoint { methods: "PATCH", path: "/patch", badge: Some(Badge::Enhanced), desc: "Same as /post, for PATCH requests.", curl: "curl -X PATCH {base}/patch -H 'Content-Type: application/json' -d '{\"key\":\"value\"}'" },
            Endpoint { methods: "DELETE", path: "/delete", badge: None, desc: "Same as /post, for DELETE requests.", curl: "curl -X DELETE {base}/delete" },
            Endpoint { methods: "HEAD", path: "/head", badge: Some(Badge::New), desc: "HEAD-only: echoes the request headers back as X-Echo-* response headers.", curl: "curl -I {base}/head" },
            Endpoint { methods: "OPTIONS", path: "/options", badge: Some(Badge::New), desc: "OPTIONS-only: echoes the request and returns an Allow header (RFC 9110 §9.3.7).", curl: "curl -X OPTIONS {base}/options" },
            Endpoint { methods: "TRACE", path: "/trace", badge: Some(Badge::New), desc: "TRACE-only: echoes the request like /anything (RFC 9110 §9.8).", curl: "curl -X TRACE {base}/trace" },
            Endpoint { methods: "QUERY", path: "/query", badge: Some(Badge::New), desc: "Echoes the request's URL args and parsed body, like /post (RFC 9430).", curl: "curl -X QUERY {base}/query -d 'select=everything'" },
        ],
    },
    Category {
        id: "anything",
        title: "Anything",
        desc: "Returns anything that is passed to the request.",
        endpoints: &[
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/OPTIONS/TRACE/QUERY", path: "/anything", badge: Some(Badge::Enhanced), desc: "Accepts any method and echoes the full request; multipart handling matches /post.", curl: "curl -X POST {base}/anything -d 'hello httpcan'" },
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/OPTIONS/TRACE/QUERY", path: "/anything/{path}", badge: Some(Badge::Enhanced), desc: "Same as /anything, with extra path segment(s) that are ignored.", curl: "curl {base}/anything/foo/bar" },
        ],
    },
    Category {
        id: "auth",
        title: "Auth",
        desc: "Auth methods.",
        endpoints: &[
            Endpoint { methods: "GET/POST", path: "/basic-auth/{user}/{passwd}", badge: None, desc: "Challenges HTTP Basic Auth with the given credentials.", curl: "curl -u user:pass {base}/basic-auth/user/pass" },
            Endpoint { methods: "GET/POST", path: "/basic-auth/{user}", badge: Some(Badge::New), desc: "Challenges HTTP Basic Auth with an empty password (username-only check).", curl: "curl -u 'user:' {base}/basic-auth/user" },
            Endpoint { methods: "GET/POST", path: "/hidden-basic-auth/{user}/{passwd}", badge: None, desc: "Like /basic-auth, but returns 404 instead of 401 on failure.", curl: "curl -u user:pass {base}/hidden-basic-auth/user/pass" },
            Endpoint { methods: "GET/POST", path: "/hidden-basic-auth/{user}", badge: Some(Badge::New), desc: "Like /basic-auth/{user}, but returns 404 instead of 401 on failure.", curl: "curl -u 'user:' {base}/hidden-basic-auth/user" },
            Endpoint { methods: "GET", path: "/bearer", badge: None, desc: "Checks for a Bearer token in the Authorization header.", curl: "curl -H 'Authorization: Bearer mytoken123' {base}/bearer" },
            Endpoint { methods: "GET", path: "/jwt-bearer", badge: Some(Badge::New), desc: "Decodes and inspects a JWT Bearer token, without verifying its signature.", curl: "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sflKxw' {base}/jwt-bearer" },
            Endpoint { methods: "GET/POST", path: "/digest-auth/{qop}/{user}/{passwd}[/{algorithm}[/{stale_after}]]", badge: Some(Badge::Enhanced), desc: "Challenges HTTP Digest Auth; adds SHA-512-256 and a qop=none legacy (RFC 2069) mode.", curl: "curl --digest -u user:pass {base}/digest-auth/auth/user/pass" },
        ],
    },
    Category {
        id: "status-codes",
        title: "Status codes",
        desc: "Generates responses with a given status code.",
        endpoints: &[
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/TRACE/OPTIONS", path: "/status/{codes}", badge: Some(Badge::Enhanced), desc: "Returns the given status, or a random one from a comma-separated list; supports ?header= injection, a custom body, and trailing path segments.", curl: "curl -i '{base}/status/429?header=Retry-After:120'" },
        ],
    },
    Category {
        id: "request-inspection",
        title: "Request inspection",
        desc: "Inspect the request data.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/headers", badge: None, desc: "Returns the request's HTTP headers.", curl: "curl {base}/headers" },
            Endpoint { methods: "GET", path: "/ip", badge: None, desc: "Returns the requester's IP address.", curl: "curl {base}/ip" },
            Endpoint { methods: "GET", path: "/user-agent", badge: None, desc: "Returns the request's User-Agent header.", curl: "curl {base}/user-agent" },
            Endpoint { methods: "ANY", path: "/method", badge: Some(Badge::New), desc: "Returns the request's HTTP method name.", curl: "curl -X LINK {base}/method" },
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/QUERY", path: "/body", badge: Some(Badge::New), desc: "Returns the request's body verbatim with mirrored headers, for any method including QUERY (RFC 9430); /echo is a compatibility alias.", curl: "curl -X POST {base}/body -d 'hello httpcan'" },
        ],
    },
    Category {
        id: "response-inspection",
        title: "Response inspection",
        desc: "Inspect the response data, like caching and headers.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/cache", badge: None, desc: "Returns 304 if If-Modified-Since or If-None-Match is present, 200 otherwise.", curl: "curl -i -H 'If-None-Match: \"abc123\"' {base}/cache" },
            Endpoint { methods: "GET", path: "/cache/{value}", badge: None, desc: "Sets a Cache-Control header for the given number of seconds.", curl: "curl -i {base}/cache/60" },
            Endpoint { methods: "GET", path: "/etag/{etag}", badge: Some(Badge::Enhanced), desc: "Validates If-Match/If-None-Match against the given ETag; supports weak W/\"...\" validators.", curl: "curl -i -H 'If-None-Match: \"abc123\"' {base}/etag/abc123" },
            Endpoint { methods: "GET/POST", path: "/response-headers", badge: Some(Badge::Enhanced), desc: "Echoes query parameters as response headers; ?body= and ?status= override the response body and status code.", curl: "curl -i '{base}/response-headers?Content-Type=text/plain&X-Foo=bar'" },
        ],
    },
    Category {
        id: "response-formats",
        title: "Response formats",
        desc: "Returns responses in different data formats.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/json", badge: None, desc: "Returns a sample JSON document.", curl: "curl {base}/json" },
            Endpoint { methods: "GET", path: "/xml", badge: None, desc: "Returns a sample XML document.", curl: "curl {base}/xml" },
            Endpoint { methods: "GET", path: "/html", badge: None, desc: "Returns a sample HTML document.", curl: "curl {base}/html" },
            Endpoint { methods: "GET", path: "/deny", badge: None, desc: "Returns the page that robots.txt disallows.", curl: "curl {base}/deny" },
            Endpoint { methods: "GET", path: "/encoding/utf8", badge: None, desc: "Returns a UTF-8 encoded page.", curl: "curl {base}/encoding/utf8" },
            Endpoint { methods: "GET", path: "/encoding/iso-8859-1", badge: Some(Badge::New), desc: "Returns an ISO-8859-1 encoded page.", curl: "curl {base}/encoding/iso-8859-1" },
            Endpoint { methods: "GET/POST", path: "/gzip", badge: Some(Badge::Enhanced), desc: "GET returns gzip-encoded JSON; POST gzip-encodes the request body.", curl: "curl {base}/gzip | gunzip" },
            Endpoint { methods: "GET/POST", path: "/deflate", badge: Some(Badge::Enhanced), desc: "GET returns deflate-encoded JSON; POST deflate-encodes the request body.", curl: "curl {base}/deflate" },
            Endpoint { methods: "GET/POST", path: "/brotli", badge: Some(Badge::Enhanced), desc: "GET returns brotli-encoded JSON; POST brotli-encodes the request body.", curl: "curl {base}/brotli" },
            Endpoint { methods: "GET/POST", path: "/zstd", badge: Some(Badge::New), desc: "GET returns Zstandard-encoded JSON; POST zstd-encodes the request body.", curl: "curl {base}/zstd" },
        ],
    },
    Category {
        id: "dynamic-data",
        title: "Dynamic data",
        desc: "Generates random and dynamic data.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/uuid", badge: None, desc: "Returns a UUIDv4.", curl: "curl {base}/uuid" },
            Endpoint { methods: "GET", path: "/base64/{value}", badge: Some(Badge::Enhanced), desc: "Decodes a standard base64 string; returns raw bytes for non-UTF-8 content instead of erroring.", curl: "curl {base}/base64/SFRUUENhbiBpcyBhd2Vzb21l" },
            Endpoint { methods: "POST", path: "/base64", badge: Some(Badge::New), desc: "Decodes the request body as base64.", curl: "curl -X POST {base}/base64 -d 'SFRUUENhbiBpcyBhd2Vzb21l'" },
            Endpoint { methods: "GET", path: "/bytes/{n}", badge: Some(Badge::Enhanced), desc: "Returns n random bytes; over the configured --max-bytes limit returns 404 instead of silently truncating.", curl: "curl {base}/bytes/1024 -o random.bin" },
            Endpoint { methods: "GET", path: "/stream-bytes/{n}", badge: Some(Badge::Enhanced), desc: "Streams n random bytes chunk by chunk; same limit behavior as /bytes.", curl: "curl {base}/stream-bytes/1024 -o random.bin" },
            Endpoint { methods: "GET", path: "/stream/{n}", badge: None, desc: "Streams n JSON lines describing the request.", curl: "curl -N {base}/stream/5" },
            Endpoint { methods: "GET", path: "/range/{numbytes}", badge: None, desc: "Streams numbytes bytes; supports Range requests.", curl: "curl -H 'Range: bytes=0-99' {base}/range/1024" },
            Endpoint { methods: "GET", path: "/links/{n}/{offset}", badge: None, desc: "Returns a page of n links, starting at offset.", curl: "curl {base}/links/5/0" },
            Endpoint { methods: "GET", path: "/links/{n}", badge: Some(Badge::New), desc: "Same as /links/{n}/{offset}, with offset defaulted to 0.", curl: "curl {base}/links/5" },
            Endpoint { methods: "GET", path: "/drip", badge: Some(Badge::Enhanced), desc: "Drips data over a duration; ?chunked=true streams with real chunked transfer-encoding.", curl: "curl '{base}/drip?duration=2&numbytes=10&delay=1&chunked=true'" },
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/TRACE", path: "/delay/{delay}", badge: None, desc: "Delays the response by up to 10 seconds.", curl: "curl {base}/delay/3" },
        ],
    },
    Category {
        id: "cookies",
        title: "Cookies",
        desc: "Creates, reads, and deletes cookies.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/cookies", badge: None, desc: "Returns the request's cookies.", curl: "curl -b 'session=abc123' {base}/cookies" },
            Endpoint { methods: "GET", path: "/cookies/set", badge: Some(Badge::Enhanced), desc: "Sets cookies from query parameters and redirects to /cookies; supports httponly/secure/samesite/domain/path/max_age attributes.", curl: "curl -i '{base}/cookies/set?session=abc123&httponly=true&samesite=lax'" },
            Endpoint { methods: "GET", path: "/cookies/set/{name}/{value}", badge: None, desc: "Sets a single cookie and redirects to /cookies.", curl: "curl -i {base}/cookies/set/session/abc123" },
            Endpoint { methods: "GET", path: "/cookies/delete", badge: None, desc: "Deletes the given cookies and redirects to /cookies.", curl: "curl -i '{base}/cookies/delete?session'" },
        ],
    },
    Category {
        id: "images",
        title: "Images",
        desc: "Returns different image formats.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/image", badge: None, desc: "Returns a random image; format negotiated via the Accept header.", curl: "curl -H 'Accept: image/webp' {base}/image -o image.webp" },
            Endpoint { methods: "GET", path: "/image/png", badge: None, desc: "Returns a PNG image.", curl: "curl {base}/image/png -o image.png" },
            Endpoint { methods: "GET", path: "/image/jpeg", badge: None, desc: "Returns a JPEG image.", curl: "curl {base}/image/jpeg -o image.jpg" },
            Endpoint { methods: "GET", path: "/image/webp", badge: None, desc: "Returns a WEBP image.", curl: "curl {base}/image/webp -o image.webp" },
            Endpoint { methods: "GET", path: "/image/svg", badge: None, desc: "Returns an SVG image.", curl: "curl {base}/image/svg -o image.svg" },
        ],
    },
    Category {
        id: "redirects",
        title: "Redirects",
        desc: "Returns different redirect responses.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/redirect/{n}", badge: None, desc: "Redirects n times before returning 200.", curl: "curl -L {base}/redirect/3" },
            Endpoint { methods: "GET", path: "/relative-redirect/{n}", badge: None, desc: "Same as /redirect/{n}, using relative Location URLs.", curl: "curl -L {base}/relative-redirect/3" },
            Endpoint { methods: "GET", path: "/absolute-redirect/{n}", badge: None, desc: "Same as /redirect/{n}, using absolute Location URLs.", curl: "curl -L {base}/absolute-redirect/3" },
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/TRACE", path: "/redirect-to", badge: Some(Badge::Enhanced), desc: "Redirects to ?url=; POST/PUT/PATCH/DELETE also accept form/JSON bodies, and browser clients see an anti-phishing interstitial instead of a silent redirect.", curl: "curl -X POST {base}/redirect-to -d 'url=https://example.com'" },
        ],
    },
    Category {
        id: "streaming",
        title: "Streaming",
        desc: "Server-Sent Events and NDJSON streaming endpoints.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/sse", badge: Some(Badge::New), desc: "Streams server-sent events; supports OpenAI-compatible chunk formats.", curl: "curl -N '{base}/sse?count=3&format=simple'" },
            Endpoint { methods: "GET", path: "/sse/{count}", badge: Some(Badge::New), desc: "Same as /sse, with a fixed event count.", curl: "curl -N {base}/sse/5" },
            Endpoint { methods: "GET", path: "/sse/{count}/{delay}", badge: Some(Badge::New), desc: "Same as /sse, with a fixed count and delay between events.", curl: "curl -N {base}/sse/5/1000" },
            Endpoint { methods: "GET", path: "/ndjson", badge: Some(Badge::New), desc: "Streams newline-delimited JSON; supports OpenAI/Ollama-compatible chunk formats.", curl: "curl -N '{base}/ndjson?count=3&format=simple'" },
            Endpoint { methods: "GET", path: "/ndjson/{count}", badge: Some(Badge::New), desc: "Same as /ndjson, with a fixed line count.", curl: "curl -N {base}/ndjson/5" },
            Endpoint { methods: "GET", path: "/ndjson/{count}/{delay}", badge: Some(Badge::New), desc: "Same as /ndjson, with a fixed count and delay between lines.", curl: "curl -N {base}/ndjson/5/1000" },
        ],
    },
    Category {
        id: "llm",
        title: "AI API Mock",
        desc: "Mock OpenAI- and Anthropic-compatible LLM APIs: point an SDK base_url at /llm and test without a paid provider.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/llm", badge: Some(Badge::New), desc: "Self-describing index: endpoints, base_url guidance for both SDK families, and the httpcan response-override contract.", curl: "curl {base}/llm" },
            Endpoint { methods: "POST", path: "/llm/v1/chat/completions", badge: Some(Badge::New), desc: "Mock OpenAI Chat Completions API: canonical chat.completion shape, SSE streaming with [DONE], and an httpcan.content override. /llm/chat/completions works as a silent alias.", curl: "curl -X POST {base}/llm/v1/chat/completions -H 'Content-Type: application/json' -d '{\"model\":\"gpt-5.6-sol\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}'" },
            Endpoint { methods: "POST", path: "/llm/v1/messages", badge: Some(Badge::New), desc: "Mock Anthropic Messages API: content blocks, stop_reason, and the full streaming event protocol (message_start through message_stop).", curl: "curl -X POST {base}/llm/v1/messages -H 'Content-Type: application/json' -d '{\"model\":\"claude-sonnet-5\",\"max_tokens\":64,\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}'" },
            Endpoint { methods: "POST", path: "/llm/v1/responses", badge: Some(Badge::New), desc: "Mock OpenAI Responses API: canonical Response object with output_text convenience field; streaming uses named SSE events (response.created through response.completed) with sequence numbers. /llm/responses works as a silent alias.", curl: "curl -X POST {base}/llm/v1/responses -H 'Content-Type: application/json' -d '{\"model\":\"gpt-5.6-sol\",\"input\":\"Hello\"}'" },
            Endpoint { methods: "POST", path: "/llm/v1/completions", badge: Some(Badge::New), desc: "Mock OpenAI legacy Completions API (text-in/text-out): echo support, n choices, and SSE streaming with [DONE]. /llm/completions works as a silent alias.", curl: "curl -X POST {base}/llm/v1/completions -H 'Content-Type: application/json' -d '{\"model\":\"gpt-5.6-sol\",\"prompt\":\"Once upon a time\"}'" },
            Endpoint { methods: "GET", path: "/llm/v1/models", badge: Some(Badge::New), desc: "Model list in the OpenAI shape, or the Anthropic shape when an anthropic-version header is present (both SDKs hit this same path).", curl: "curl {base}/llm/v1/models" },
            Endpoint { methods: "GET", path: "/llm/v1/models/{model}", badge: Some(Badge::New), desc: "Single-model lookup that always returns 200 and echoes the requested id — any model name is valid.", curl: "curl {base}/llm/v1/models/gpt-5.6-sol" },
        ],
    },
    Category {
        id: "oauth2",
        title: "OAuth 2.0 Mock",
        desc: "Mock authorization server: RFC 6749 four grants with PKCE, one-time codes, rotating refresh tokens, and RFC 8414 discovery.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/oauth2", badge: Some(Badge::New), desc: "Self-describing index: endpoints, supported grants, PKCE methods, and the mock's state semantics.", curl: "curl {base}/oauth2" },
            Endpoint { methods: "GET/POST", path: "/oauth2/authorize", badge: Some(Badge::New), desc: "Authorization endpoint with a consent page. response_type=code issues a one-time authorization code (PKCE supported); response_type=token (implicit, legacy) returns the access token in the redirect fragment.", curl: "curl '{base}/oauth2/authorize?response_type=code&client_id=cli&redirect_uri=https%3A%2F%2Fexample.com%2Fcb&state=xyz'" },
            Endpoint { methods: "POST", path: "/oauth2/token", badge: Some(Badge::New), desc: "Token endpoint: authorization_code (+PKCE), password, refresh_token (rotating), and client_credentials grants; Basic and form-body client authentication.", curl: "curl -X POST {base}/oauth2/token -d 'grant_type=client_credentials&client_id=cli&client_secret=s3cr3t'" },
            Endpoint { methods: "GET/POST", path: "/oauth2/userinfo", badge: Some(Badge::New), desc: "Bearer-protected resource: verifies the access token and returns the mock identity (sub/email/client_id/scope).", curl: "curl {base}/oauth2/userinfo -H 'Authorization: Bearer <token>'" },
        ],
    },
    Category {
        id: "observability",
        title: "Observability",
        desc: "Health checks and instance identification.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/healthz", badge: Some(Badge::New), desc: "Liveness probe: returns 200 whenever the server is up.", curl: "curl {base}/healthz" },
            Endpoint { methods: "GET", path: "/tags", badge: Some(Badge::New), desc: "Returns all HTTPCAN_* environment variables, for instance identification.", curl: "curl {base}/tags" },
            Endpoint { methods: "GET", path: "/tags/{name}", badge: Some(Badge::New), desc: "Returns a single HTTPCAN_* environment variable by name.", curl: "curl {base}/tags/VERSION" },
        ],
    },
];

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn badge_html(badge: Option<Badge>) -> String {
    match badge {
        None => String::new(),
        Some(b) => format!(
            r#"<span class="{}" title="{}">{}</span>"#,
            b.css_class(),
            b.title(),
            b.label()
        ),
    }
}

/// Renders a copy-to-clipboard button. `command` is the exact text placed on
/// the clipboard (kept in `data-curl`, which the client script reads).
fn copy_button_html(command: &str, label: &str) -> String {
    format!(
        r#"<button type="button" class="copy-btn" data-curl="{}" aria-label="{}">Copy</button>"#,
        escape_html(command),
        escape_html(label)
    )
}

/// Same-origin path for the homepage's "Try" button, or `None` when the
/// endpoint can't be exercised by a plain browser GET (it needs a request
/// body, custom headers, credentials, or a non-GET method). Harmless curl
/// display flags (`-i`, `-L`, `-N`, `-o file`, `| gunzip`) are ignored
/// because `fetch` follows redirects, buffers streams and decodes gzip itself.
fn try_path(ep: &Endpoint) -> Option<String> {
    if !ep.methods.starts_with("GET") {
        return None;
    }
    let mut rest = ep.curl.strip_prefix("curl ")?;
    for flag in ["-i ", "-L ", "-N "] {
        rest = rest.strip_prefix(flag).unwrap_or(rest);
    }
    let rest = rest.strip_suffix(" | gunzip").unwrap_or(rest);
    let rest = rest.find(" -o ").map_or(rest, |i| &rest[..i]);
    let rest = rest.trim_matches('\'');
    let path = rest.strip_prefix("{base}")?;
    if path.is_empty() || path.contains([' ', '<', '\'']) {
        return None;
    }
    Some(path.to_string())
}

fn methods_label(methods: &str) -> String {
    if methods == "ANY" {
        "Any method".to_string()
    } else {
        methods.replace('/', ", ")
    }
}

fn render_sidebar(total: usize, enhanced: usize, new: usize) -> String {
    let mut s = String::new();
    s.push_str(
        "<aside class=\"ref-side\">\n\
         <label class=\"filter-label\" for=\"ep-filter\">Filter endpoints</label>\n\
         <input id=\"ep-filter\" type=\"search\" placeholder=\"Path or keyword\" autocomplete=\"off\" spellcheck=\"false\" aria-describedby=\"ep-filter-hint\">\n\
         <p id=\"ep-filter-hint\" class=\"filter-hint\">Press <kbd>/</kbd> to jump here.</p>\n\
         <nav class=\"cat-nav\" aria-label=\"Endpoint categories\">\n",
    );
    for cat in CATEGORIES {
        let _ = writeln!(
            s,
            "<a href=\"#{}\">{}<span class=\"count\">{}</span></a>",
            cat.id,
            escape_html(cat.title),
            cat.endpoints.len()
        );
    }
    let _ = write!(
        s,
        "</nav>\n\
         <div class=\"legend\">\n\
         <p>Badges compare each of the {total} endpoints with httpbin.org.</p>\n\
         <p>{new_badge} Not in httpbin.org ({new})</p>\n\
         <p>{enh_badge} In httpbin.org, fixed or extended here ({enhanced})</p>\n\
         <p>No badge: works the same as httpbin.org</p>\n\
         </div>\n\
         </aside>\n",
        new_badge = badge_html(Some(Badge::New)),
        enh_badge = badge_html(Some(Badge::Enhanced)),
    );
    s
}

fn render_categories(base: &str) -> String {
    let mut s = String::new();
    for cat in CATEGORIES {
        let _ = write!(
            s,
            "<section class=\"ep-cat\" id=\"{id}\">\n<h3><a class=\"anchor\" href=\"#{id}\">{title}</a></h3>\n<p class=\"cat-desc\">{desc}</p>\n<ul class=\"ep-list\">\n",
            id = cat.id,
            title = escape_html(cat.title),
            desc = escape_html(cat.desc)
        );
        for ep in cat.endpoints {
            let search = format!("{} {} {}", ep.path, ep.methods, ep.desc).to_lowercase();
            let try_btn = match try_path(ep) {
                Some(path) => format!(
                    r#"<button type="button" class="try-btn" data-path="{p}" aria-label="Send GET {p} from this page">Try</button>"#,
                    p = escape_html(&path)
                ),
                None => String::new(),
            };
            let curl = ep.curl.replace("{base}", base);
            let _ = writeln!(
                s,
                "<li data-search=\"{search}\"><div class=\"ep-head\"><code class=\"path\">{path}</code><span class=\"methods\">{methods}</span>{badge}<span class=\"ep-actions\">{try_btn}{copy}</span></div><p class=\"desc\">{desc}</p></li>",
                search = escape_html(&search),
                path = escape_html(ep.path),
                methods = escape_html(&methods_label(ep.methods)),
                badge = badge_html(ep.badge),
                copy = copy_button_html(&curl, &format!("Copy curl command for {}", ep.path)),
                desc = escape_html(ep.desc)
            );
        }
        s.push_str("</ul>\n</section>\n");
    }
    s
}

fn badge_counts() -> (usize, usize, usize) {
    let mut enhanced = 0;
    let mut new = 0;
    // Total counts spec paths (not catalog entries) so the "N-endpoint
    // superset" claim stays arithmetically consistent with /openapi.json —
    // e.g. the compact digest-auth entry covers three spec paths.
    let total = super::openapi::spec_path_count();
    for cat in CATEGORIES {
        for ep in cat.endpoints {
            match ep.badge {
                Some(Badge::Enhanced) => enhanced += 1,
                Some(Badge::New) => new += 1,
                None => {}
            }
        }
    }
    (total, enhanced, new)
}

/// Homepage typefaces, embedded in the binary (SIL OFL 1.1; licenses live
/// next to the files in `static/fonts/`) so the page makes no third-party
/// requests and works offline / air-gapped.
const FONT_SCHIBSTED_GROTESK: &[u8] = include_bytes!("../../static/fonts/schibsted-grotesk.woff2");
const FONT_MARTIAN_MONO: &[u8] = include_bytes!("../../static/fonts/martian-mono.woff2");

/// `/fonts/schibsted-grotesk.woff2` and `/fonts/martian-mono.woff2`. The
/// homepage references them with a `?v=<version>` query, so they can be
/// cached as immutable.
pub async fn font_handler(req: HttpRequest) -> Result<HttpResponse> {
    let body = match req.path() {
        "/fonts/schibsted-grotesk.woff2" => FONT_SCHIBSTED_GROTESK,
        "/fonts/martian-mono.woff2" => FONT_MARTIAN_MONO,
        _ => return Ok(HttpResponse::NotFound().finish()),
    };
    Ok(HttpResponse::Ok()
        .content_type("font/woff2")
        .insert_header(("Cache-Control", "public, max-age=31536000, immutable"))
        .body(body))
}

fn font_faces(version: &str) -> String {
    format!(
        r#"@font-face {{ font-family: "Schibsted Grotesk"; src: url("/fonts/schibsted-grotesk.woff2?v={version}") format("woff2"); font-weight: 400 900; font-style: normal; font-display: swap; }}
@font-face {{ font-family: "Martian Mono"; src: url("/fonts/martian-mono.woff2?v={version}") format("woff2"); font-weight: 100 800; font-stretch: 75% 112.5%; font-style: normal; font-display: swap; }}"#
    )
}

const STYLE: &str = r#"
:root {
    color-scheme: light dark;
    --paper: #F5F7F8;
    --raised: #FFFFFF;
    --ink: #17202B;
    --muted: #56616D;
    --tin: #DCE2E6;
    --tin-edge: #A9B4BD;
    --string: #1E88E5;
    --send-hover: #1565C0;
    --sans: "Schibsted Grotesk", ui-sans-serif, system-ui, sans-serif;
    --mono: "Martian Mono", ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
}
@media (prefers-color-scheme: dark) {
    :root {
        --paper: #161D26;
        --raised: #1C2530;
        --ink: #E8EDF1;
        --muted: #9AA6B2;
        --tin: #26303C;
        --tin-edge: #45525F;
        --string: #1E88E5;
        --send-hover: #1E88E5;
    }
}
*, *::before, *::after { box-sizing: border-box; }
[hidden] { display: none !important; }
html { scroll-behavior: smooth; -webkit-text-size-adjust: 100%; }
body { margin: 0; background: var(--paper); color: var(--ink); font: 400 1.125rem/1.55 var(--sans); }
a { color: inherit; text-decoration: underline; text-decoration-color: var(--tin-edge); text-decoration-thickness: 1px; text-underline-offset: 0.2em; }
a:hover { text-decoration-color: var(--string); }
:focus-visible { outline: 2px solid var(--string); outline-offset: 2px; border-radius: 3px; }
code, kbd, pre { font-family: var(--mono); font-stretch: 87.5%; }
kbd { font-size: 0.8em; padding: 0.05rem 0.35rem; border: 1px solid var(--tin-edge); border-radius: 4px; background: var(--raised); }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
.wrap { max-width: 72rem; margin: 0 auto; padding: 0 1.5rem; }

/* Header */
.site-head { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 0.5rem 2rem; padding: 1.5rem 0 0; }
.wordmark { font-family: var(--mono); font-weight: 800; font-stretch: 112.5%; font-size: 1.3125rem; letter-spacing: -0.02em; text-decoration: none; }
.site-nav { display: flex; flex-wrap: wrap; gap: 0.25rem 1.5rem; font-size: 1rem; }

/* Hero */
.hero { display: grid; grid-template-columns: minmax(0, 7fr) minmax(0, 5fr); gap: 2rem 4rem; align-items: end; padding: 4.5rem 0 3rem; }
.hero h1 { margin: 0 0 1.25rem; font-size: clamp(2.5rem, 1.5rem + 4vw, 4.5rem); font-weight: 800; line-height: 1; letter-spacing: -0.035em; max-width: 12ch; text-wrap: balance; }
.lede { margin: 0; max-width: 34em; font-size: 1.3125rem; line-height: 1.5; }
.install p { margin: 0 0 0.6rem; font-size: 1rem; font-weight: 600; }
.cmd { display: flex; align-items: center; gap: 0.75rem; margin-top: 0.5rem; padding: 0.5rem 0.5rem 0.5rem 0.9rem; background: var(--raised); border: 1px solid var(--tin-edge); border-radius: 8px; }
.cmd code { flex: 1; min-width: 0; white-space: pre-wrap; overflow-wrap: anywhere; font-size: 0.8125rem; line-height: 1.5; }

/* Live console: two cans joined by a string */
.console { display: grid; grid-template-columns: minmax(0, 5fr) 6rem minmax(0, 7fr); align-items: start; }
.can { min-width: 0; background: var(--raised); border: 1.5px solid var(--tin-edge); border-radius: 14px; overflow: hidden; }
.can-rim { display: flex; justify-content: space-between; align-items: baseline; gap: 1rem; padding: 0.65rem 1rem; font-size: 0.9375rem; font-weight: 700; border-bottom: 1.5px solid var(--tin-edge); background-color: var(--tin); background-image: repeating-linear-gradient(90deg, transparent 0 4px, rgba(127, 127, 127, 0.16) 4px 5px); }
.can-rim .meta { font-weight: 500; color: var(--muted); font-size: 0.875rem; font-variant-numeric: tabular-nums; }
.can-body { padding: 1rem; }
.can pre { margin: 0; font-size: 0.8125rem; line-height: 1.65; white-space: pre-wrap; overflow-wrap: anywhere; }
#req-view, #res-view { max-height: 17rem; overflow: auto; }
#res-view .res-head { color: var(--muted); }
#res-view img { display: block; max-width: 100%; max-height: 16rem; margin-top: 0.25rem; border-radius: 6px; }
.try-line { display: flex; margin-bottom: 1rem; border: 1.5px solid var(--tin-edge); border-radius: 8px; overflow: hidden; background: var(--paper); }
.try-line:focus-within { outline: 2px solid var(--string); outline-offset: 2px; }
.try-line .verb { display: flex; align-items: center; padding: 0 0.75rem; font: 700 0.8125rem/1 var(--mono); background: var(--tin); }
.try-line input { flex: 1; min-width: 0; padding: 0.65rem 0.75rem; border: 0; background: transparent; color: inherit; font: 400 0.875rem/1.2 var(--mono); font-stretch: 87.5%; }
.try-line input:focus { outline: none; }
.send-btn { padding: 0 1.1rem; border: 0; background: var(--ink); color: var(--paper); font: 700 0.9375rem/1 var(--sans); cursor: pointer; }
.send-btn:hover { background: var(--send-hover); }
.string { display: block; width: 100%; overflow: visible; }
.string path { fill: none; vector-effect: non-scaling-stroke; stroke-linecap: round; }
.string .line { stroke: var(--string); stroke-width: 2.5; }
.string .pulse { stroke: var(--ink); stroke-width: 6; stroke-dasharray: 8 200; stroke-dashoffset: 8; opacity: 0; }
.string-h { height: 9rem; margin-top: 1.3rem; }
.string-v { display: none; width: 4rem; height: 3.5rem; margin-left: 2.5rem; }
.is-sending .string .pulse { opacity: 1; animation: travel 0.6s cubic-bezier(0.45, 0, 0.25, 1) forwards; }
@keyframes travel { to { stroke-dashoffset: -100; } }
.console-note { margin: 1.25rem 0 0; max-width: 46em; color: var(--muted); font-size: 1rem; }
.console-note .cmd { margin-top: 0.6rem; max-width: 40rem; color: var(--ink); }

/* Endpoint reference */
.ref-intro { padding-top: 5.5rem; }
.ref { display: grid; grid-template-columns: 15rem minmax(0, 1fr); gap: 3.5rem; padding-top: 0.5rem; }
.ref-side { position: sticky; top: 1.5rem; align-self: start; max-height: calc(100vh - 3rem); overflow-y: auto; padding: 0.25rem 0.5rem 1rem 0.25rem; font-size: 0.9375rem; }
.filter-label { display: block; margin-bottom: 0.4rem; font-weight: 700; }
#ep-filter { width: 100%; padding: 0.6rem 0.75rem; border: 1.5px solid var(--tin-edge); border-radius: 8px; background: var(--raised); color: inherit; font: 400 1rem/1.2 var(--sans); }
.filter-hint { margin: 0.4rem 0 1.25rem; color: var(--muted); font-size: 0.875rem; }
.cat-nav a { display: flex; justify-content: space-between; gap: 1rem; padding: 0.3rem 0; text-decoration: none; }
.cat-nav a:hover { color: var(--string); }
.cat-nav .count { color: var(--muted); font-variant-numeric: tabular-nums; }
.legend { margin-top: 1.5rem; padding-top: 1rem; border-top: 1.5px solid var(--tin); color: var(--muted); font-size: 0.875rem; }
.legend p { margin: 0 0 0.6rem; }
.ref-main { min-width: 0; }
h2 { margin: 0 0 0.5rem; font-size: 2.25rem; font-weight: 800; line-height: 1.1; letter-spacing: -0.025em; scroll-margin-top: 1.5rem; }
.section-intro { margin: 0 0 1.5rem; max-width: 40em; color: var(--muted); }
.ep-cat { padding-top: 2rem; scroll-margin-top: 0.5rem; }
.ep-cat h3 { margin: 0; font-size: 1.5rem; font-weight: 700; letter-spacing: -0.015em; }
.ep-cat h3 .anchor { text-decoration: none; }
.ep-cat h3 .anchor:hover { color: var(--string); }
.cat-desc { margin: 0.15rem 0 0.75rem; color: var(--muted); font-size: 1rem; }
.ep-list { margin: 0; padding: 0; list-style: none; }
.ep-list li { padding: 0.85rem 0; border-top: 1px solid var(--tin); }
.ep-head { display: flex; flex-wrap: wrap; align-items: center; gap: 0.35rem 0.75rem; }
.ep-head .path { font-size: 0.9375rem; font-weight: 600; overflow-wrap: anywhere; }
.ep-head .methods { color: var(--muted); font-size: 0.875rem; }
.ep-actions { display: flex; gap: 0.4rem; margin-left: auto; }
.ep-list .desc { margin: 0.3rem 0 0; max-width: 42em; font-size: 1rem; }
#ep-empty { padding: 2rem 0; color: var(--muted); }
.badge { display: inline-block; padding: 0.2rem 0.5rem; border-radius: 999px; font: 600 0.75rem/1.2 var(--sans); white-space: nowrap; }
.badge-new { background: var(--ink); color: var(--paper); }
.badge-enhanced { padding: calc(0.2rem - 1px) calc(0.5rem - 1px); border: 1px solid var(--ink); color: var(--ink); }
.copy-btn, .try-btn { padding: 0.35rem 0.65rem; border: 1px solid var(--tin-edge); border-radius: 6px; background: var(--raised); color: var(--ink); font: 600 0.8125rem/1 var(--sans); cursor: pointer; white-space: nowrap; }
.copy-btn:hover, .try-btn:hover { background: var(--tin); }
.copy-btn.copied { border-color: var(--ink); background: var(--ink); color: var(--paper); }

/* What HTTPCan adds */
.adds { padding-top: 5.5rem; }
.adds dl { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0 4rem; margin: 1.5rem 0 0; }
.adds dt { font-weight: 700; font-size: 1.125rem; }
.adds dd { margin: 0.2rem 0 1.75rem; max-width: 34em; color: var(--muted); font-size: 1rem; }
.adds code { font-size: 0.875em; color: var(--ink); }

/* Footer */
.site-foot { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 0.75rem 2rem; margin-top: 3rem; padding: 1.75rem 0 4rem; border-top: 1.5px solid var(--tin); color: var(--muted); font-size: 0.9375rem; }
.site-foot p { margin: 0; }
.site-foot nav { display: flex; flex-wrap: wrap; gap: 0.25rem 1.5rem; }

@media (max-width: 960px) {
    .hero { grid-template-columns: minmax(0, 1fr); padding-top: 3rem; }
    .ref-intro { padding-top: 4rem; }
    .ref { grid-template-columns: minmax(0, 1fr); gap: 0.5rem; }
    .ref-side { position: static; max-height: none; overflow: visible; padding: 0; }
    .cat-nav { display: flex; flex-wrap: wrap; gap: 0.25rem 1.25rem; }
    .cat-nav a { gap: 0.4rem; }
    .adds dl { grid-template-columns: minmax(0, 1fr); }
}
@media (max-width: 720px) {
    .console { grid-template-columns: minmax(0, 1fr); }
    .string-h { display: none; }
    .string-v { display: block; }
    h2 { font-size: 1.875rem; }
}
@media (hover: hover) {
    .ep-list li .copy-btn, .ep-list li .try-btn { color: var(--muted); }
    .ep-list li:hover .copy-btn, .ep-list li:hover .try-btn, .ep-list li .copy-btn:focus-visible, .ep-list li .try-btn:focus-visible { color: var(--ink); }
    .ep-list li .copy-btn.copied { color: var(--paper); }
}
@media (prefers-reduced-motion: reduce) {
    html { scroll-behavior: auto; }
    .is-sending .string .pulse { animation: none; opacity: 0; }
}
"#;

/// Vanilla JS, no dependencies: copy-to-clipboard (with a
/// `document.execCommand` fallback for non-secure contexts such as plain HTTP
/// on a LAN), the live request console, and the endpoint filter.
const SCRIPT: &str = r#"
(function () {
    'use strict';
    var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    function copyText(text) {
        if (navigator.clipboard && window.isSecureContext) {
            return navigator.clipboard.writeText(text);
        }
        var ta = document.createElement('textarea');
        ta.value = text;
        ta.style.position = 'fixed';
        ta.style.opacity = '0';
        document.body.appendChild(ta);
        ta.focus();
        ta.select();
        try { document.execCommand('copy'); } finally { document.body.removeChild(ta); }
        return Promise.resolve();
    }

    // Live console
    var consoleEl = document.getElementById('try');
    var form = document.getElementById('try-form');
    var input = document.getElementById('try-path');
    var reqView = document.getElementById('req-view');
    var resView = document.getElementById('res-view');
    var resMeta = document.getElementById('res-meta');
    var curlCode = document.getElementById('try-curl');
    var curlBtn = document.getElementById('try-curl-btn');
    var REASONS = { 200: 'OK', 201: 'Created', 202: 'Accepted', 204: 'No Content', 206: 'Partial Content', 304: 'Not Modified', 400: 'Bad Request', 401: 'Unauthorized', 403: 'Forbidden', 404: 'Not Found', 405: 'Method Not Allowed', 418: "I'm a teapot", 429: 'Too Many Requests', 500: 'Internal Server Error', 502: 'Bad Gateway', 503: 'Service Unavailable' };
    var SHOWN_HEADERS = ['content-type', 'content-length', 'content-encoding', 'cache-control', 'etag', 'x-httpcan-version'];
    var current = null;
    var imageUrl = null;

    function normalizePath(p) {
        p = p.trim();
        if (!p) p = '/get';
        if (p.charAt(0) !== '/') p = '/' + p;
        return p;
    }
    function titleCase(h) {
        return h.replace(/(^|-)([a-z])/g, function (_, d, c) { return d + c.toUpperCase(); });
    }
    function wait(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }

    async function send() {
        var path = normalizePath(input.value);
        input.value = path;
        var curl = "curl '" + location.origin + path + "'";
        curlCode.textContent = curl;
        curlBtn.setAttribute('data-curl', curl);
        reqView.textContent = 'GET ' + path + ' HTTP/1.1\nHost: ' + location.host;
        resMeta.textContent = 'Sending';
        resView.textContent = '';

        if (current) current.abort();
        var mine = new AbortController();
        current = mine;
        consoleEl.classList.remove('is-sending');
        void consoleEl.offsetWidth; // restart the string animation
        consoleEl.classList.add('is-sending');

        var started = performance.now();
        try {
            var res = await fetch(path, { signal: mine.signal, cache: 'no-store' });
            var blob = await res.blob();
            var ms = Math.round(performance.now() - started);
            // Let the pulse reach the far can before the answer appears.
            if (!reduceMotion) await wait(Math.max(0, 600 - ms));
            if (mine !== current) return;

            var ct = res.headers.get('content-type') || '';
            var status = (res.status + ' ' + (res.statusText || REASONS[res.status] || '')).trim();
            var meta = status + ', ' + ms + ' ms';
            if (res.redirected) meta += ', redirected to ' + new URL(res.url).pathname;
            resMeta.textContent = meta;

            var head = document.createElement('span');
            head.className = 'res-head';
            head.textContent = 'HTTP/1.1 ' + status + '\n' + SHOWN_HEADERS
                .filter(function (h) { return res.headers.get(h); })
                .map(function (h) { return titleCase(h) + ': ' + res.headers.get(h); })
                .join('\n') + '\n\n';
            resView.textContent = '';
            resView.appendChild(head);

            if (imageUrl) { URL.revokeObjectURL(imageUrl); imageUrl = null; }
            if (/^image\//.test(ct)) {
                imageUrl = URL.createObjectURL(blob);
                var img = new Image();
                img.alt = 'Image returned by ' + path;
                img.src = imageUrl;
                resView.appendChild(img);
            } else if (/^text\/|json|xml|javascript|event-stream|ndjson/.test(ct) || blob.size === 0) {
                var body = await blob.text();
                if (/json/.test(ct)) {
                    try {
                        var data = JSON.parse(body);
                        body = JSON.stringify(data, null, 2);
                        // Echo endpoints report the headers the server actually
                        // received: show those as the request.
                        if (data && data.headers && typeof data.headers === 'object' && !Array.isArray(data.headers)) {
                            reqView.textContent = 'GET ' + path + ' HTTP/1.1\n' + Object.keys(data.headers).map(function (k) {
                                var v = data.headers[k];
                                return k + ': ' + (Array.isArray(v) ? v.join(', ') : v);
                            }).join('\n');
                        }
                    } catch (_) { /* not valid JSON; show as text */ }
                }
                var limit = 8000;
                if (body.length > limit) body = body.slice(0, limit) + '\n\n(' + (body.length - limit) + ' more characters not shown)';
                resView.appendChild(document.createTextNode(body || '(empty body)'));
            } else {
                resView.appendChild(document.createTextNode('(' + blob.size + ' bytes of ' + (ct || 'binary data') + ')'));
            }
        } catch (err) {
            if (err.name === 'AbortError') return;
            resMeta.textContent = 'No response';
            resView.textContent = 'The request did not complete (' + err.message + '). Check the path and press Send again.';
        } finally {
            if (mine === current) consoleEl.classList.remove('is-sending');
        }
    }

    form.addEventListener('submit', function (e) { e.preventDefault(); send(); });

    document.addEventListener('click', function (e) {
        var copyBtn = e.target.closest('.copy-btn');
        if (copyBtn) {
            copyText(copyBtn.getAttribute('data-curl')).then(function () {
                copyBtn.textContent = 'Copied';
                copyBtn.classList.add('copied');
                setTimeout(function () {
                    copyBtn.textContent = 'Copy';
                    copyBtn.classList.remove('copied');
                }, 1200);
            });
            return;
        }
        var tryBtn = e.target.closest('.try-btn');
        if (tryBtn) {
            input.value = tryBtn.getAttribute('data-path');
            consoleEl.scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', block: 'start' });
            send();
        }
    });

    // Endpoint filter
    var filter = document.getElementById('ep-filter');
    var rows = Array.prototype.slice.call(document.querySelectorAll('.ep-list li'));
    var cats = Array.prototype.slice.call(document.querySelectorAll('.ep-cat'));
    var empty = document.getElementById('ep-empty');
    filter.addEventListener('input', function () {
        var q = filter.value.trim().toLowerCase();
        var shown = 0;
        rows.forEach(function (li) {
            var hit = !q || li.getAttribute('data-search').indexOf(q) !== -1;
            li.hidden = !hit;
            if (hit) shown++;
        });
        cats.forEach(function (c) { c.hidden = !c.querySelector('li:not([hidden])'); });
        empty.hidden = shown !== 0;
        if (!shown) empty.textContent = 'No endpoints match “' + filter.value.trim() + '”. Clear the filter to see all of them.';
    });
    document.addEventListener('keydown', function (e) {
        var tag = document.activeElement && document.activeElement.tagName;
        if (e.key === '/' && !e.metaKey && !e.ctrlKey && tag !== 'INPUT' && tag !== 'TEXTAREA' && tag !== 'SELECT') {
            e.preventDefault();
            filter.focus();
        }
    });

    send();
})();
"#;

/// The string between the two cans: a sagging line plus the pulse that
/// travels along it while a request is in flight. `pathLength="100"` keeps
/// the dash animation independent of the rendered size.
fn string_svg(class: &str, d: &str) -> String {
    format!(
        r#"<svg class="string {class}" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true" focusable="false"><path class="line" d="{d}" pathLength="100"/><path class="pulse" d="{d}" pathLength="100"/></svg>"#
    )
}

fn render_homepage(canonical_url: &str, base: &str, host: &str, version: &str) -> String {
    let (total, enhanced, new) = badge_counts();
    let docker_cmd = "docker run -p 8080:8080 ghcr.io/seedvector/httpcan:latest";
    let cargo_cmd = "cargo install httpcan && httpcan";
    let try_default = "/get?hello=can";
    let try_curl = format!("curl '{base}{try_default}'");
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>HTTPCan &mdash; httpbin Alternative with AI &amp; OAuth2 Mocks</title>
<meta name="description" content="A Rust superset of httpbin.org: {total} drop-in endpoints, mock OpenAI/Anthropic LLM APIs, and a full OAuth 2.0 server — for testing HTTP clients and AI agents.">
<link rel="canonical" href="{canonical_url}">
<link rel="icon" type="image/png" href="/favicon.png">
<link rel="preload" href="/fonts/schibsted-grotesk.woff2?v={version}" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="/fonts/martian-mono.woff2?v={version}" as="font" type="font/woff2" crossorigin>
<style>{font_faces}{style}</style>
</head>
<body>
<div class="wrap">
<header class="site-head">
<a class="wordmark" href="/">HTTPCan</a>
<nav class="site-nav" aria-label="Site">
<a href="#endpoints">Endpoints</a>
<a href="/openapi.json">OpenAPI spec</a>
<a href="https://github.com/seedvector/httpcan">GitHub</a>
</nav>
</header>

<main>
<section class="hero">
<div>
<h1>Send a request. See exactly what arrived.</h1>
<p class="lede">HTTPCan is an HTTP test server for checking clients, proxies and AI agents. It covers every <a href="https://httpbin.org">httpbin.org</a> endpoint, fixes {enhanced} of them, adds {new} more, and includes mock OpenAI, Anthropic and OAuth&nbsp;2.0 servers.</p>
</div>
<div class="install" id="self-host">
<p>Run your own copy</p>
<div class="cmd"><code>{docker_cmd_html}</code>{docker_copy}</div>
<div class="cmd"><code>{cargo_cmd_html}</code>{cargo_copy}</div>
</div>
</section>

<section id="try" aria-label="Send a request from this page">
<div class="console">
<form class="can" id="try-form">
<div class="can-rim"><span>Request</span></div>
<div class="can-body">
<div class="try-line">
<span class="verb">GET</span>
<label class="sr-only" for="try-path">Path to request</label>
<input id="try-path" name="path" value="{try_default}" autocomplete="off" spellcheck="false">
<button type="submit" class="send-btn">Send</button>
</div>
<pre id="req-view">GET {try_default} HTTP/1.1
Host: {host}</pre>
</div>
</form>
{string_h}{string_v}
<div class="can">
<div class="can-rim"><span>Response</span><span class="meta" id="res-meta"></span></div>
<div class="can-body">
<pre id="res-view" aria-live="polite">Press Send to see what HTTPCan returns.</pre>
<noscript><p>JavaScript is off, so this page can&rsquo;t send requests. Run the command below in a terminal instead.</p></noscript>
</div>
</div>
</div>
<div class="console-note">
<p>This page sent a real request to this server when it loaded. Change the path and press Send, or press Try next to any endpoint below. The same request from a terminal:</p>
<div class="cmd"><code id="try-curl">{try_curl_html}</code><button type="button" class="copy-btn" id="try-curl-btn" data-curl="{try_curl_html}" aria-label="Copy curl command for this request">Copy</button></div>
</div>
</section>

<section class="ref-intro">
<h2 id="endpoints">Endpoints</h2>
<p class="section-intro">Every example runs against this server ({base}), so you can paste it straight into a terminal.</p>
</section>
<div class="ref">
{sidebar}
<div class="ref-main">
{categories}
<p id="ep-empty" hidden></p>
</div>
</div>

<section class="adds" id="why-httpcan">
<h2>What HTTPCan adds to httpbin.org</h2>
<dl>
<dt>Mock AI APIs</dt>
<dd>OpenAI- and Anthropic-compatible endpoints under <code>/llm</code>. Point an SDK&rsquo;s <code>base_url</code> at them to test your client, streaming included, without a paid key.</dd>
<dt>Mock OAuth&nbsp;2.0 server</dt>
<dd>All four RFC 6749 grants with PKCE, one-time codes, rotating refresh tokens, a Bearer-protected userinfo endpoint and RFC 8414 discovery, under <code>/oauth2</code>.</dd>
<dt>Streaming endpoints</dt>
<dd><code>/sse</code> and <code>/ndjson</code> send OpenAI- and Ollama-shaped chunks, so you can test streaming parsers.</dd>
<dt>Safer redirects</dt>
<dd>Browsers that open <code>/redirect-to</code> see a confirmation page instead of a silent 302, so the endpoint can&rsquo;t be abused as an open redirect.</dd>
<dt>Headers kept intact</dt>
<dd>Duplicate and non-ASCII request headers are echoed back as sent instead of being dropped.</dd>
<dt>Proxy headers filtered</dt>
<dd>About 100 reverse-proxy and CDN headers are removed from echoed responses. Add your own with <code>--exclude-headers</code>.</dd>
<dt>Health checks</dt>
<dd><code>/healthz</code> for liveness probes, <code>/tags</code> for instance identification, and <code>Server-Timing</code> and <code>X-Httpcan-Version</code> headers on every response.</dd>
<dt>Fast</dt>
<dd>Built with Rust and Actix Web. In our benchmarks it led comparable test servers on throughput and tail latency.</dd>
</dl>
</section>
</main>

<footer class="site-foot">
<p>HTTPCan {version}, MIT License.</p>
<nav aria-label="Project">
<a href="/openapi.json">OpenAPI spec</a>
<a href="https://github.com/seedvector/httpcan">Source on GitHub</a>
<a href="https://github.com/seedvector/httpcan/issues">Report an issue</a>
</nav>
</footer>
</div>
<script>{script}</script>
</body>
</html>"##,
        canonical_url = canonical_url,
        font_faces = font_faces(version),
        style = STYLE,
        total = total,
        enhanced = enhanced,
        new = new,
        docker_cmd_html = escape_html(docker_cmd),
        docker_copy = copy_button_html(docker_cmd, "Copy Docker command"),
        cargo_cmd_html = escape_html(cargo_cmd),
        cargo_copy = copy_button_html(cargo_cmd, "Copy Cargo command"),
        try_default = escape_html(try_default),
        try_curl_html = escape_html(&try_curl),
        host = escape_html(host),
        string_h = string_svg("string-h", "M0 2 Q50 95 100 2"),
        string_v = string_svg("string-v", "M30 0 Q95 50 30 100"),
        sidebar = render_sidebar(total, enhanced, new),
        categories = render_categories(base),
        base = escape_html(base),
        version = version,
        script = SCRIPT,
    )
}

/// Escapes `|` so a value is safe inside a Markdown table cell. Cell content
/// here is single-line, so only the column-delimiting pipe needs escaping.
fn md_table_escape(cell: &str) -> String {
    cell.replace('|', "\\|")
}

/// Renders the homepage as Markdown for AI agents that send
/// `Accept: text/markdown` (Content-Type: text/markdown). Mirrors the HTML
/// page — intro, quick start, highlights, and every endpoint grouped by
/// category with runnable curl examples resolved against `base` — so agents
/// get clean text instead of scraping dense HTML. See
/// <https://developers.cloudflare.com/fundamentals/reference/markdown-for-agents/>.
fn render_markdown(base: &str, version: &str) -> String {
    let (total, enhanced, new) = badge_counts();
    let mut s = String::new();

    let _ = writeln!(
        s,
        "# HTTPCan\n\nA modern, high-performance superset of [httpbin.org](https://httpbin.org) for testing HTTP clients, proxies, and AI agents — with mock OpenAI/Anthropic LLM APIs and a full OAuth 2.0 server. Built with Rust and Actix Web.\n"
    );
    let _ = writeln!(s, "- [OpenAPI spec]({base}/openapi.json)");
    let _ = writeln!(
        s,
        "- [Source on GitHub](https://github.com/seedvector/httpcan)\n"
    );

    let _ = writeln!(s, "## Quick Start\n");
    let _ = writeln!(
        s,
        "You're on a live instance — `{base}`. Try an endpoint now:\n"
    );
    let _ = writeln!(s, "```sh");
    let _ = writeln!(s, "curl {base}/get");
    let _ = writeln!(s, "```\n");
    let _ = writeln!(s, "Want your own? Run HTTPCan locally or in production:\n");
    let _ = writeln!(s, "```sh");
    let _ = writeln!(s, "# Docker");
    let _ = writeln!(
        s,
        "docker run -p 8080:8080 ghcr.io/seedvector/httpcan:latest\n"
    );
    let _ = writeln!(s, "# Cargo");
    let _ = writeln!(s, "cargo install httpcan && httpcan\n");
    let _ = writeln!(s, "curl http://localhost:8080/get");
    let _ = writeln!(s, "```\n");

    let _ = writeln!(s, "## Why HTTPCan\n");
    let _ = writeln!(
        s,
        "HTTPCan is a **{total}-endpoint superset of httpbin.org**: every httpbin.org endpoint is covered and drop-in compatible, plus {new} endpoints httpbin.org doesn't have and {enhanced} it has but httpcan fixes or extends.\n"
    );
    let _ = writeln!(s, "- **AI API mock** — OpenAI- and Anthropic-compatible `/llm` endpoints: point any SDK `base_url` at it (streaming included) and test AI clients without a paid provider.");
    let _ = writeln!(s, "- **OAuth 2.0 mock** — a full authorization server at `/oauth2`: four RFC 6749 grants with PKCE, one-time codes, rotating refresh tokens, Bearer-protected userinfo, and RFC 8414 discovery.");
    let _ = writeln!(s, "- **AI-friendly streaming** — native `/sse` and `/ndjson` endpoints with OpenAI/Ollama-compatible chunk formats.");
    let _ = writeln!(s, "- **Measured performance** — in our benchmarks against comparable HTTP testing servers, HTTPCan led on both throughput and tail latency.");
    let _ = writeln!(s, "- **Anti-phishing redirects** — browser clients hitting `/redirect-to` see a confirmation page instead of a silent 302, closing an open-redirect abuse vector.");
    let _ = writeln!(s, "- **Correct header handling** — duplicate and non-ASCII request headers are preserved instead of being dropped or crashing the server.");
    let _ = writeln!(s, "- **Safer by default** — built-in filtering strips ~100 reverse-proxy/CDN headers from echoed responses, with `--exclude-headers` for more.");
    let _ = writeln!(
        s,
        "- **Cloud-native observability** — `/healthz` liveness probe, `/tags` instance identification, and `Server-Timing`/`X-Httpcan-Version` on every response.\n"
    );
    let _ = writeln!(
        s,
        "> **{enhanced}** Enhanced (httpbin has it; httpcan fixes/extends) · **{new}** New (not in httpbin.org) · no badge = drop-in compatible.\n"
    );

    let _ = writeln!(s, "## Endpoints\n");
    let _ = writeln!(s, "All examples below target this instance (`{base}`).\n");
    for cat in CATEGORIES {
        let _ = writeln!(s, "### {}\n", cat.title);
        let _ = writeln!(s, "{}\n", cat.desc);
        let _ = writeln!(s, "| Method | Path | Description | Example |");
        let _ = writeln!(s, "| --- | --- | --- | --- |");
        for ep in cat.endpoints {
            let badge = match ep.badge {
                Some(Badge::Enhanced) => " _(Enhanced)_",
                Some(Badge::New) => " _(New)_",
                None => "",
            };
            let desc = format!("{}{badge}", ep.desc);
            let example = ep.curl.replace("{base}", base);
            let _ = writeln!(
                s,
                "| `{}` | `{}` | {} | `{}` |",
                md_table_escape(ep.methods),
                md_table_escape(ep.path),
                md_table_escape(&desc),
                md_table_escape(&example)
            );
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(s, "---\n");
    let _ = writeln!(
        s,
        "HTTPCan · version {version} · {total} endpoints · MIT License\n"
    );

    s
}

/// Returns true when the client explicitly asks for Markdown via
/// `Accept: text/markdown`. Mirrors the Accept parsing used by the `/image`
/// endpoint; browsers and crawlers that don't request Markdown keep getting
/// the HTML page.
fn wants_markdown(req: &HttpRequest) -> bool {
    req.headers()
        .get("accept")
        .and_then(|h| h.to_str().ok())
        .map(|a| a.to_lowercase().contains("text/markdown"))
        .unwrap_or(false)
}

/// Adds RFC 8288 / RFC 9727 `Link` headers to the homepage response so agents
/// can discover HTTPCan's machine-readable resources without scraping HTML:
/// the API catalog (`api-catalog`, RFC 9727 §3), the OpenAPI spec
/// (`service-desc`, RFC 8631) which also describes the resource (`describedby`,
/// RFC 8288), and the human-readable docs (`service-doc`, RFC 8631). Targets
/// are relative refs resolved against the request URI, and each header carries
/// a single relation type for broad parser compatibility.
fn add_homepage_link_headers(
    mut res: actix_web::HttpResponseBuilder,
) -> actix_web::HttpResponseBuilder {
    res.append_header(("Link", "</.well-known/api-catalog>; rel=\"api-catalog\""));
    res.append_header(("Link", "</openapi.json>; rel=\"service-desc\""));
    res.append_header(("Link", "</>; rel=\"service-doc\""));
    res.append_header(("Link", "</openapi.json>; rel=\"describedby\""));
    res
}

/// Homepage: a fully server-rendered, static HTML page describing HTTPCan and
/// listing every endpoint grouped by category, with a compatibility badge
/// relative to httpbin.org and a one-click "Copy" button for a ready-to-run
/// curl example.
///
/// Content negotiation (Markdown for Agents): a request carrying
/// `Accept: text/markdown` gets the same content rendered as Markdown with
/// `Content-Type: text/markdown` and an estimated `x-markdown-tokens` count,
/// so AI agents receive clean text instead of scraping HTML. Every other
/// request — browsers, search engines, AI crawlers — gets the HTML page (see
/// `/openapi.json` for the machine-readable spec).
///
/// Every homepage response also carries RFC 8288 / RFC 9727 `Link` headers
/// (api-catalog, service-desc, service-doc, describedby) so agents can discover
/// the machine-readable resources without scraping the page.
pub async fn root_handler(req: HttpRequest, config: web::Data<AppConfig>) -> Result<HttpResponse> {
    // User override: `static/index.html` replaces the built-in homepage at
    // `/`. It forfeits the dynamic parts (origin-resolved curl examples,
    // markdown negotiation, RFC 8288 Link headers, canonical URL) — the
    // operator's file is served verbatim.
    if let Some(file) = static_override(&config, "index.html") {
        return Ok(file.into_response(&req));
    }
    // `base` mirrors the scheme/host the visitor actually used, so curl
    // examples and copy-paste links stay usable as-is (an https visitor gets
    // https examples, a plain-http visitor — e.g. local dev, or a self-hosted
    // instance without TLS — gets http examples that actually work).
    let connection_info = req.connection_info();
    let base = format!("{}://{}", connection_info.scheme(), connection_info.host());
    // The canonical link is a pure SEO signal, not something a user copies
    // to run — it should stay stable regardless of how this particular
    // request arrived, so it honors `scheme_override` instead of mirroring it.
    let canonical_url = format!("{}/", resolved_base(&req, &config));
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");

    if wants_markdown(&req) {
        let body = render_markdown(&base, version);
        // Rough estimate (~4 chars/token); exact counts need a BPE tokenizer.
        let tokens = body.chars().count() / 4;
        return Ok(add_homepage_link_headers(HttpResponse::Ok())
            .content_type("text/markdown; charset=utf-8")
            .insert_header(("x-markdown-tokens", tokens.to_string()))
            .body(body));
    }

    Ok(add_homepage_link_headers(HttpResponse::Ok())
        .content_type("text/html; charset=utf-8")
        .body(render_homepage(
            &canonical_url,
            &base,
            connection_info.host(),
            version,
        )))
}

/// `/favicon.png` — embedded in the binary so the default experience is fully
/// self-contained (the built-in homepage references it in `<head>`). A file
/// named `favicon.png` in the static assets directory overrides it, e.g. for
/// custom-branded deployments.
const EMBEDDED_FAVICON: &[u8] = include_bytes!("../../static/favicon.png");

pub async fn favicon_handler(
    req: HttpRequest,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    if let Some(file) = static_override(&config, "favicon.png") {
        return Ok(file.into_response(&req));
    }
    Ok(HttpResponse::Ok()
        .content_type(mime::IMAGE_PNG)
        .body(EMBEDDED_FAVICON))
}
