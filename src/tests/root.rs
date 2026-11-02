//! Homepage (/)

use super::*;

/// GET / serves the static homepage as HTML by default — 200, text/html
/// content-type, a body containing an <html> tag — for every Accept value
/// except an explicit `text/markdown` (covered by the markdown-negotiation
/// tests below). Browsers, search engines, and AI crawlers all see the same
/// crawlable HTML.
#[actix_web::test]
async fn root_always_serves_html() {
    let app = test::init_service(create_app(cfg())).await;
    for accept in ["text/html", "application/json", "*/*"] {
        let req = test::TestRequest::get()
            .uri("/")
            .insert_header(("Accept", accept))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .expect("content-type header")
            .to_str()
            .unwrap();
        assert!(
            ct.starts_with("text/html"),
            "expected text/html content-type for Accept: {accept}, got {ct}"
        );
        let body = test::read_body(resp).await;
        let body_str = std::str::from_utf8(&body).expect("utf-8 html body");
        assert!(
            body_str.to_lowercase().contains("<html"),
            "html body must contain an <html> tag"
        );
    }
}

/// GET / with no Accept header at all also serves the homepage.
#[actix_web::test]
async fn root_serves_html_without_accept_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .unwrap();
    assert!(ct.starts_with("text/html"));
}

/// GET / with `Accept: text/markdown` returns the homepage as Markdown
/// (Content-Type: text/markdown) instead of HTML — clean text for AI agents,
/// with curl examples resolved against the request's own origin.
#[actix_web::test]
async fn root_serves_markdown_for_markdown_accept() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/")
        .insert_header(("Host", "example.com"))
        .insert_header(("Accept", "text/markdown"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .unwrap();
    assert!(
        ct.starts_with("text/markdown"),
        "expected text/markdown content-type, got {ct}"
    );
    let body = test::read_body(resp).await;
    let md = std::str::from_utf8(&body).expect("utf-8 markdown body");
    assert!(md.contains("# HTTPCan"), "markdown has the page title");
    assert!(md.contains("## Endpoints"), "markdown lists endpoints");
    assert!(
        md.contains("| Method | Path |"),
        "markdown renders an endpoint table"
    );
    assert!(
        !md.contains("{base}"),
        "markdown must substitute the origin into curl examples: {md}"
    );
    assert!(
        md.contains("http://example.com/get"),
        "markdown curl examples target the request origin: {md}"
    );
    assert!(
        !md.to_lowercase().contains("<html"),
        "markdown response must not contain HTML"
    );
}

/// An Accept header that lists `text/markdown` alongside other types still
/// opts into the Markdown view (browsers never advertise text/markdown).
#[actix_web::test]
async fn root_serves_markdown_when_listed_alongside_other_types() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/")
        .insert_header(("Accept", "text/markdown, text/html"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .unwrap();
    assert!(
        ct.starts_with("text/markdown"),
        "Accept listing text/markdown should yield Markdown, got {ct}"
    );
}

/// The Markdown response carries an `x-markdown-tokens` header with a positive
/// estimated token count, per the Markdown-for-Agents spec.
#[actix_web::test]
async fn root_markdown_includes_token_count_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/")
        .insert_header(("Accept", "text/markdown"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let tokens = resp
        .headers()
        .get("x-markdown-tokens")
        .expect("x-markdown-tokens header")
        .to_str()
        .unwrap()
        .parse::<usize>()
        .expect("token count is an integer");
    assert!(tokens > 0, "token count should be positive, got {tokens}");
}

/// GET / (homepage) carries RFC 8288 / RFC 9727 `Link` headers advertising the
/// machine-readable resources — api-catalog, service-desc, service-doc, and
/// describedby — with correct targets, on both the HTML and Markdown
/// representations so discovery works regardless of negotiated content type.
#[actix_web::test]
async fn root_homepage_has_discovery_link_headers() {
    let app = test::init_service(create_app(cfg())).await;

    // HTML (default) response.
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let links: Vec<String> = resp
        .headers()
        .get_all("link")
        .map(|h| h.to_str().unwrap().to_string())
        .collect();
    assert!(!links.is_empty(), "homepage must carry Link headers");
    let joined = links.join(", ");
    assert!(
        joined.contains(r#"</.well-known/api-catalog>; rel="api-catalog""#),
        "missing api-catalog link: {joined}"
    );
    assert!(
        joined.contains(r#"</openapi.json>; rel="service-desc""#),
        "missing service-desc link: {joined}"
    );
    assert!(
        joined.contains(r#"</>; rel="service-doc""#),
        "missing service-doc link: {joined}"
    );
    assert!(
        joined.contains(r#"</openapi.json>; rel="describedby""#),
        "missing describedby link: {joined}"
    );

    // The Markdown representation carries the same discovery links.
    let req = test::TestRequest::get()
        .uri("/")
        .insert_header(("Accept", "text/markdown"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let links: Vec<String> = resp
        .headers()
        .get_all("link")
        .map(|h| h.to_str().unwrap().to_string())
        .collect();
    let joined = links.join(", ");
    assert!(
        joined.contains(r#"</.well-known/api-catalog>; rel="api-catalog""#),
        "markdown homepage must also carry discovery links: {joined}"
    );
}

/// The homepage lists real endpoints (grouped by category) and links to the
/// machine-readable OpenAPI spec.
#[actix_web::test]
async fn root_homepage_lists_endpoints_and_links_to_openapi() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).expect("utf-8 html body");
    assert!(body_str.contains("/get"), "homepage should list /get");
    assert!(body_str.contains("/sse"), "homepage should list /sse");
    assert!(
        body_str.contains("/openapi.json"),
        "homepage should link to the OpenAPI spec"
    );
}

/// `canonical_scheme` must only affect the SEO-facing canonical link, not the
/// user-facing copy-curl examples: a plain-http visitor should still see
/// `http://` in the curl examples (so they're actually runnable as-is), even
/// when the instance is configured to always advertise `https` for
/// canonical/sitemap/robots purposes.
#[actix_web::test]
async fn root_canonical_honors_override_while_examples_mirror_request() {
    let app = test::init_service(create_app(
        cfg().canonical_scheme(config::SchemeOverride::Https),
    ))
    .await;
    let req = test::TestRequest::get()
        .uri("/")
        .insert_header(("Host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).expect("utf-8 html body");
    assert!(
        body_str.contains(r#"<link rel="canonical" href="https://example.com/">"#),
        "canonical link must honor canonical_scheme(Https): {body_str}"
    );
    assert!(
        body_str.contains("http://example.com/get"),
        "copy-curl examples must mirror the actual (plain-http) request, not the override: {body_str}"
    );
    assert!(
        !body_str.contains("https://example.com/get"),
        "copy-curl examples must not be forced to https by canonical_scheme: {body_str}"
    );
}

/// Each endpoint on the homepage has a "Copy" button with a ready-to-run
/// curl example (resolved against the request's own origin) attached via
/// `data-curl`, plus the client-side script that wires up copy-to-clipboard.
#[actix_web::test]
async fn root_homepage_has_copy_curl_buttons() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/")
        .insert_header(("Host", "example.com"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).expect("utf-8 html body");

    assert!(
        body_str.contains(r#"class="copy-btn""#),
        "homepage should render a copy-curl button for each endpoint"
    );
    assert!(
        body_str.contains(r#"data-curl="curl http://example.com/uuid""#),
        "copy button for /uuid should target the request's own origin"
    );
    assert!(
        body_str.contains("navigator.clipboard"),
        "homepage should include the copy-to-clipboard script"
    );
}

/// Drift guard: every piece of shared homepage copy (hero, self-host
/// commands, highlights, badge legend, footer) appears in both the HTML page
/// and the Markdown view, compared as plain text.
#[actix_web::test]
async fn root_markdown_and_html_carry_the_same_copy() {
    let app = test::init_service(create_app(cfg())).await;
    let fetch = |accept: &'static str| {
        test::TestRequest::get()
            .uri("/")
            .insert_header(("Host", "example.com"))
            .insert_header(("Accept", accept))
            .to_request()
    };
    let html = test::read_body(test::call_service(&app, fetch("text/html")).await).await;
    let md = test::read_body(test::call_service(&app, fetch("text/markdown")).await).await;
    let html = std::str::from_utf8(&html).unwrap();
    let md = std::str::from_utf8(&md).unwrap();

    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    // HTML → text: drop tags, decode the entities the page uses.
    let mut html_text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            // Inline tags (<code>, <a>) must not split words; block
            // elements are already separated by newlines in the markup.
            '>' => in_tag = false,
            _ if !in_tag => html_text.push(c),
            _ => {}
        }
    }
    let html_text = squash(
        &html_text
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&rsquo;", "’")
            .replace("&hellip;", "…")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace('\u{a0}', " "),
    );
    // Markdown → text: links to their text, emphasis and code ticks dropped.
    let md_text = squash(
        &crate::handlers::root::inline_plain(md)
            .replace("**", "")
            .replace('\u{a0}', " "),
    );

    for phrase in
        crate::handlers::root::shared_homepage_copy("http://example.com", env!("CARGO_PKG_VERSION"))
    {
        let phrase = squash(&phrase);
        assert!(html_text.contains(&phrase), "HTML homepage lacks: {phrase}");
        assert!(
            md_text.contains(&phrase),
            "Markdown homepage lacks: {phrase}"
        );
    }
}
