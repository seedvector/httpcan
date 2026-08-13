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
            Endpoint { methods: "GET", path: "/get", badge: None, desc: "Returns query args, headers, origin IP, and URL.", curl: "curl '{base}/get?greeting=hello'" },
            Endpoint { methods: "POST", path: "/post", badge: Some(Badge::Enhanced), desc: "Echoes body/form/JSON; same-named multipart fields collect into an array.", curl: "curl -X POST {base}/post -H 'Content-Type: application/json' -d '{\"key\":\"value\"}'" },
            Endpoint { methods: "PUT", path: "/put", badge: Some(Badge::Enhanced), desc: "Same as /post, for PUT requests.", curl: "curl -X PUT {base}/put -H 'Content-Type: application/json' -d '{\"key\":\"value\"}'" },
            Endpoint { methods: "PATCH", path: "/patch", badge: Some(Badge::Enhanced), desc: "Same as /post, for PATCH requests.", curl: "curl -X PATCH {base}/patch -H 'Content-Type: application/json' -d '{\"key\":\"value\"}'" },
            Endpoint { methods: "DELETE", path: "/delete", badge: None, desc: "Same as /post, for DELETE requests.", curl: "curl -X DELETE {base}/delete" },
            Endpoint { methods: "ANY", path: "/method", badge: Some(Badge::New), desc: "Echoes back whichever HTTP method name was used to call it.", curl: "curl -X LINK {base}/method" },
            Endpoint { methods: "HEAD", path: "/head", badge: Some(Badge::New), desc: "HEAD-only endpoint; mirrors request headers back as X-Echo-* response headers.", curl: "curl -I {base}/head" },
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/QUERY", path: "/echo", badge: Some(Badge::New), desc: "Reflects the request body and headers verbatim, for any method including QUERY (RFC 9430).", curl: "curl -X POST {base}/echo -d 'hello httpcan'" },
        ],
    },
    Category {
        id: "anything",
        title: "Anything",
        desc: "Returns anything that is passed to the request.",
        endpoints: &[
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/TRACE/QUERY", path: "/anything", badge: Some(Badge::Enhanced), desc: "Accepts any method and echoes the full request; multipart handling matches /post.", curl: "curl -X POST {base}/anything -d 'hello httpcan'" },
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/TRACE/QUERY", path: "/anything/{path}", badge: Some(Badge::Enhanced), desc: "Same as /anything, with an extra path segment that is ignored.", curl: "curl {base}/anything/foo/bar" },
        ],
    },
    Category {
        id: "auth",
        title: "Auth",
        desc: "Auth methods.",
        endpoints: &[
            Endpoint { methods: "GET/POST", path: "/basic-auth/{user}/{passwd}", badge: None, desc: "Challenges HTTP Basic Auth with the given credentials.", curl: "curl -u user:pass {base}/basic-auth/user/pass" },
            Endpoint { methods: "GET/POST", path: "/basic-auth/{user}", badge: Some(Badge::New), desc: "Basic Auth with an empty password (username-only check).", curl: "curl -u 'user:' {base}/basic-auth/user" },
            Endpoint { methods: "GET/POST", path: "/hidden-basic-auth/{user}/{passwd}", badge: None, desc: "Like /basic-auth, but returns 404 instead of 401 on failure.", curl: "curl -u user:pass {base}/hidden-basic-auth/user/pass" },
            Endpoint { methods: "GET/POST", path: "/hidden-basic-auth/{user}", badge: Some(Badge::New), desc: "Hidden Basic Auth with an empty password.", curl: "curl -u 'user:' {base}/hidden-basic-auth/user" },
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
            Endpoint { methods: "GET/POST", path: "/response-headers", badge: Some(Badge::Enhanced), desc: "Echoes query params as response headers; ?body= and ?status= override the response body and status code.", curl: "curl -i '{base}/response-headers?Content-Type=text/plain&X-Foo=bar'" },
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
            Endpoint { methods: "GET", path: "/robots.txt", badge: None, desc: "Returns a robots.txt that disallows /deny.", curl: "curl {base}/robots.txt" },
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
            Endpoint { methods: "GET", path: "/base64/{value}", badge: Some(Badge::Enhanced), desc: "Decodes a URL-safe base64 string; returns raw bytes for non-UTF-8 content instead of erroring.", curl: "curl {base}/base64/SFRUUENhbiBpcyBhd2Vzb21l" },
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
            Endpoint { methods: "GET", path: "/cookies/set", badge: Some(Badge::Enhanced), desc: "Sets cookies from query params; supports httponly/secure/samesite/domain/path/max_age attributes.", curl: "curl -i '{base}/cookies/set?session=abc123&httponly=true&samesite=lax'" },
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
            Endpoint { methods: "GET/POST/PUT/PATCH/DELETE/TRACE", path: "/redirect-to", badge: Some(Badge::Enhanced), desc: "Redirects to ?url=; POST/PUT/PATCH also accept form/JSON bodies, and browser clients see an anti-phishing interstitial instead of a silent redirect.", curl: "curl -X POST {base}/redirect-to -d 'url=https://example.com'" },
        ],
    },
    Category {
        id: "streaming",
        title: "Streaming",
        desc: "Server-Sent Events and NDJSON streaming endpoints.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/sse", badge: Some(Badge::New), desc: "Server-Sent Events stream; supports OpenAI/Ollama-compatible chunk formats.", curl: "curl -N '{base}/sse?count=3&format=simple'" },
            Endpoint { methods: "GET", path: "/sse/{count}", badge: Some(Badge::New), desc: "Same as /sse, with a fixed event count.", curl: "curl -N {base}/sse/5" },
            Endpoint { methods: "GET", path: "/sse/{count}/{delay}", badge: Some(Badge::New), desc: "Same as /sse, with a fixed count and delay between events.", curl: "curl -N {base}/sse/5/1000" },
            Endpoint { methods: "GET", path: "/ndjson", badge: Some(Badge::New), desc: "Newline-delimited JSON stream; supports OpenAI/Ollama-compatible chunk formats.", curl: "curl -N '{base}/ndjson?count=3&format=simple'" },
            Endpoint { methods: "GET", path: "/ndjson/{count}", badge: Some(Badge::New), desc: "Same as /ndjson, with a fixed line count.", curl: "curl -N {base}/ndjson/5" },
            Endpoint { methods: "GET", path: "/ndjson/{count}/{delay}", badge: Some(Badge::New), desc: "Same as /ndjson, with a fixed count and delay between lines.", curl: "curl -N {base}/ndjson/5/1000" },
        ],
    },
    Category {
        id: "observability",
        title: "Observability",
        desc: "Health checks and instance identification.",
        endpoints: &[
            Endpoint { methods: "GET", path: "/healthz", badge: Some(Badge::New), desc: "Liveness probe; returns 200 whenever the server is up.", curl: "curl {base}/healthz" },
            Endpoint { methods: "GET", path: "/tags", badge: Some(Badge::New), desc: "Lists HTTPCAN_* environment variables, for instance identification.", curl: "curl {base}/tags" },
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
            r#" <span class="{}" title="{}">{}</span>"#,
            b.css_class(),
            b.title(),
            b.label()
        ),
    }
}

/// Renders the copy-to-clipboard button for an endpoint's example curl
/// command. `base` is the current request's origin, substituted into the
/// endpoint's `{base}`-templated curl string before HTML-escaping.
fn copy_button_html(curl_template: &str, base: &str) -> String {
    let curl = curl_template.replace("{base}", base);
    format!(
        r#" <button type="button" class="copy-btn" data-curl="{}" title="Copy curl example" aria-label="Copy curl example">Copy</button>"#,
        escape_html(&curl)
    )
}

fn render_toc() -> String {
    let mut s = String::new();
    s.push_str("<nav class=\"toc\" aria-label=\"Endpoint categories\">\n");
    for cat in CATEGORIES {
        let _ = writeln!(s, "<a href=\"#{}\">{}</a>", cat.id, escape_html(cat.title));
    }
    s.push_str("</nav>\n");
    s
}

fn render_categories(base: &str) -> String {
    let mut s = String::new();
    for cat in CATEGORIES {
        let _ = write!(
            s,
            "<section id=\"{}\">\n<h2>{} <a class=\"hash\" href=\"#{}\">#</a></h2>\n<p class=\"cat-desc\">{}</p>\n<ul class=\"endpoint-list\">\n",
            cat.id,
            escape_html(cat.title),
            cat.id,
            escape_html(cat.desc)
        );
        for ep in cat.endpoints {
            let _ = writeln!(
                s,
                "<li><code class=\"method\">{}</code> <code class=\"path\">{}</code>{}{}<span class=\"desc\">{}</span></li>",
                escape_html(ep.methods),
                escape_html(ep.path),
                badge_html(ep.badge),
                copy_button_html(ep.curl, base),
                escape_html(ep.desc)
            );
        }
        s.push_str("</ul>\n</section>\n");
    }
    s
}

fn badge_counts() -> (usize, usize, usize) {
    let mut enhanced = 0;
    let mut new = 0;
    let mut total = 0;
    for cat in CATEGORIES {
        for ep in cat.endpoints {
            total += 1;
            match ep.badge {
                Some(Badge::Enhanced) => enhanced += 1,
                Some(Badge::New) => new += 1,
                None => {}
            }
        }
    }
    (total, enhanced, new)
}

const STYLE: &str = r#"
:root { color-scheme: light dark; }
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body {
    margin: 0 auto;
    padding: 0 1.25rem 4rem;
    max-width: 880px;
    font: 17px/1.55 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    color: #1f2328;
    background: #fff;
}
@media (prefers-color-scheme: dark) {
    body { color: #e6edf3; background: #0d1117; }
    a { color: #58a6ff; }
    .badge-enhanced { background: #1f3a5f; color: #9ecbff; }
    .badge-new { background: #123a24; color: #7ee2a8; }
    code, .kbd { background: #161b22; }
    .highlights, .quick-start pre { background: #161b22; border-color: #30363d; }
    footer { border-color: #30363d; }
    .copy-btn { background: #161b22; border-color: #30363d; color: #e6edf3; }
    .copy-btn:hover { background: #21262d; }
    .copy-btn.copied { background: #123a24; border-color: #2ea043; color: #7ee2a8; }
}
a { color: #0969da; text-decoration: none; }
a:hover { text-decoration: underline; }
header.hero { padding: 2.5rem 0 1rem; }
header.hero h1 { font-size: 2.5rem; margin: 0 0 0.25rem; letter-spacing: -0.02em; }
header.hero p.tagline { margin: 0 0 1rem; opacity: 0.85; font-size: 1.05rem; }
nav.top-links { display: flex; flex-wrap: wrap; gap: 0.75rem 1.25rem; font-size: 0.95rem; }
h2 { font-size: 1.5rem; margin: 2.5rem 0 0.25rem; scroll-margin-top: 1rem; }
h2 .hash { opacity: 0; margin-left: 0.4rem; font-weight: 400; }
h2:hover .hash { opacity: 0.5; }
p.cat-desc { margin: 0 0 0.75rem; opacity: 0.75; }
code { background: #f6f8fa; padding: 0.1rem 0.4rem; border-radius: 4px; font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace; font-size: 0.85em; }
.endpoint-list { list-style: none; margin: 0; padding: 0; }
.endpoint-list li { display: flex; flex-wrap: wrap; align-items: baseline; gap: 0.5rem; padding: 0.4rem 0; border-bottom: 1px dashed rgba(127,127,127,0.25); }
.endpoint-list li:last-child { border-bottom: none; }
.endpoint-list code.method { opacity: 0.6; font-size: 0.75em; min-width: 3.5em; }
.endpoint-list code.path { font-weight: 600; }
.endpoint-list span.desc { flex-basis: 100%; opacity: 0.85; font-size: 0.95em; }
.badge { display: inline-block; font-size: 0.7em; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; padding: 0.1rem 0.45rem; border-radius: 999px; }
.badge-enhanced { background: #ddf0ff; color: #0550ae; }
.badge-new { background: #dafbe1; color: #116329; }
.copy-btn { font: inherit; font-size: 0.7em; font-weight: 600; letter-spacing: 0.02em; padding: 0.1rem 0.5rem; border-radius: 5px; border: 1px solid rgba(127,127,127,0.35); background: #f6f8fa; color: inherit; cursor: pointer; line-height: 1.6; opacity: 0; transition: opacity 0.1s; }
.endpoint-list li:hover .copy-btn, .copy-btn:focus-visible, .copy-btn.copied { opacity: 1; }
.copy-btn:hover { background: #eaeef2; }
.copy-btn.copied { background: #dafbe1; border-color: #2ea043; color: #116329; }
.toc { display: flex; flex-wrap: wrap; gap: 0.5rem 1rem; margin: 1rem 0 2rem; font-size: 0.95rem; }
.highlights, .quick-start pre { background: #f6f8fa; border: 1px solid rgba(127,127,127,0.2); border-radius: 8px; padding: 1rem 1.25rem; }
.highlights ul { margin: 0.5rem 0 0; padding-left: 1.2rem; }
.highlights li { margin: 0.35rem 0; }
.legend { display: flex; flex-wrap: wrap; gap: 1.25rem; align-items: center; font-size: 0.9rem; opacity: 0.9; margin: 1rem 0 0; }
pre { overflow-x: auto; margin: 0.5rem 0; }
pre code { background: none; padding: 0; font-size: 0.85em; }
footer { margin-top: 3rem; padding-top: 1.5rem; border-top: 1px solid rgba(127,127,127,0.2); font-size: 0.9rem; opacity: 0.85; }
footer p { margin: 0.35rem 0; }
"#;

/// Vanilla JS, no dependencies: copies the clicked endpoint's `data-curl`
/// attribute to the clipboard, with a `document.execCommand` fallback for
/// non-secure contexts (e.g. testing over plain HTTP on a LAN).
const SCRIPT: &str = r#"
function httpcanCopyText(text) {
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
document.addEventListener('click', function (event) {
    var btn = event.target.closest('.copy-btn');
    if (!btn) return;
    httpcanCopyText(btn.getAttribute('data-curl')).then(function () {
        var previous = btn.textContent;
        btn.textContent = 'Copied!';
        btn.classList.add('copied');
        setTimeout(function () {
            btn.textContent = previous;
            btn.classList.remove('copied');
        }, 1200);
    });
});
"#;

fn render_homepage(canonical_url: &str, base: &str, version: &str) -> String {
    let (_total, enhanced, new) = badge_counts();
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>HTTPCan &mdash; HTTP Request &amp; Response Testing Service</title>
<meta name="description" content="HTTPCan is a modern, high-performance superset of httpbin.org for testing HTTP clients, proxies, and AI agents: fully httpbin-compatible, plus SSE/NDJSON streaming, an anti-phishing redirect interstitial, and cloud-native observability.">
<link rel="canonical" href="{canonical_url}">
<link rel="icon" type="image/png" href="/favicon.png">
<style>{style}</style>
</head>
<body>
<header class="hero">
<h1>HTTPCan</h1>
<p class="tagline">A modern, high-performance superset of <a href="https://httpbin.org">httpbin.org</a> for testing HTTP clients, proxies, and AI agents &mdash; built with Rust and Actix Web.</p>
<nav class="top-links">
<a href="#quick-start">Quick Start</a>
<a href="#why-httpcan">Why HTTPCan</a>
<a href="/openapi.json">OpenAPI Spec</a>
<a href="https://github.com/seedvector/httpcan">GitHub</a>
</nav>
</header>

<section id="quick-start">
<h2>Quick Start <a class="hash" href="#quick-start">#</a></h2>
<pre><code># Docker
docker run -p 8080:8080 ghcr.io/seedvector/httpcan:latest

# Cargo
cargo install httpcan &amp;&amp; httpcan

curl http://localhost:8080/get</code></pre>
</section>

<section id="why-httpcan">
<h2>Why HTTPCan <a class="hash" href="#why-httpcan">#</a></h2>
<div class="highlights">
<p>HTTPCan is a drop-in replacement for httpbin.org: every httpbin.org endpoint is covered, plus {new} additional endpoints and {enhanced} bug-fixed or extended ones (see the badges below). On top of that:</p>
<ul>
<li><strong>Anti-phishing redirects</strong> &mdash; browser clients hitting <code>/redirect-to</code> see a confirmation page instead of a silent 302, closing an open-redirect abuse vector.</li>
<li><strong>AI-friendly streaming</strong> &mdash; native <code>/sse</code> and <code>/ndjson</code> endpoints with OpenAI/Ollama-compatible chunk formats.</li>
<li><strong>Cloud-native observability</strong> &mdash; <code>/healthz</code> liveness probe, <code>/tags</code> instance identification, and <code>Server-Timing</code>/<code>X-Httpcan-Version</code> on every response.</li>
<li><strong>Correct header handling</strong> &mdash; duplicate and non-ASCII request headers are preserved instead of being dropped or crashing the server.</li>
<li><strong>Safer by default</strong> &mdash; built-in filtering strips ~100 reverse-proxy/CDN headers from echoed responses, with <code>--exclude-headers</code> for more.</li>
</ul>
<p class="legend">
<span><span class="badge badge-enhanced">Enhanced</span> &nbsp;{enhanced} endpoints httpbin has, but httpcan fixes or extends</span>
<span><span class="badge badge-new">New</span> &nbsp;{new} endpoints not available in httpbin.org</span>
<span>no badge &nbsp;drop-in compatible with httpbin.org</span>
</p>
</div>
</section>

<section id="endpoints">
<h2>Endpoints <a class="hash" href="#endpoints">#</a></h2>
<p class="cat-desc">Each endpoint has a <strong>Copy</strong> button with a ready-to-run curl example (with parameters filled in) targeting this instance.</p>
{toc}
{categories}
</section>

<footer>
<p>HTTPCan &middot; version {version} &middot; MIT License</p>
<p><a href="/openapi.json">OpenAPI spec</a> &middot; <a href="https://github.com/seedvector/httpcan">Source on GitHub</a> &middot; <a href="https://github.com/seedvector/httpcan/issues">Report an issue</a></p>
</footer>
<script>{script}</script>
</body>
</html>"##,
        canonical_url = canonical_url,
        style = STYLE,
        enhanced = enhanced,
        new = new,
        toc = render_toc(),
        categories = render_categories(base),
        version = version,
        script = SCRIPT,
    )
}

/// Homepage: a fully server-rendered, static HTML page describing HTTPCan and
/// listing every endpoint grouped by category, with a compatibility badge
/// relative to httpbin.org and a one-click "Copy" button for a ready-to-run
/// curl example. Always returns HTML regardless of the `Accept` header —
/// search engines, AI crawlers, and browsers alike get the same crawlable
/// content (see `/openapi.json` for the machine-readable spec).
pub async fn root_handler(req: HttpRequest) -> Result<HttpResponse> {
    let connection_info = req.connection_info();
    let base = format!("{}://{}", connection_info.scheme(), connection_info.host());
    let canonical_url = format!("{base}/");
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(render_homepage(&canonical_url, &base, version)))
}
