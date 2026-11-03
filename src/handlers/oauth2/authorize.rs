//! `GET/POST /oauth2/authorize` — the authorization endpoint (RFC 6749 §3.1)
//! covering both redirection flows:
//! - `response_type=code` (§4.1): approval mints a one-time code delivered in
//!   the redirect query;
//! - `response_type=token` (§4.2, implicit — legacy compatibility): approval
//!   mints an access token delivered in the redirect **fragment**; no refresh
//!   token is ever issued (§4.2.2 MUST NOT).
//!
//! Validation order mirrors §4.1.1/§4.1.2.1: a missing or non-http(s)
//! `redirect_uri`, or a missing `client_id`, must NOT be redirected — the
//! resource owner gets an error page (§3.1.2.4). Every other failure is
//! reported to the client by redirecting with `error` (+`state`).

use super::{is_http_url, oauth_redirect, sign, state_param, BearerClaims, CodeClaims};
use crate::handlers::page::{html_escape, icon, page_shell, warn_icon};
use actix_web::{web, HttpResponse};
use std::collections::HashMap;

/// `GET /oauth2/authorize` — validate the request (RFC 6749 §4.1.1) and
/// render the consent page.
pub async fn oauth2_authorize_get_handler(
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let query = query.into_inner();
    let redirect_uri = query.get("redirect_uri").map(String::as_str).unwrap_or("");
    if !is_http_url(redirect_uri) {
        return authorize_error_page(
            "Invalid redirect_uri",
            "The redirect_uri parameter is missing or not an absolute http(s) URL. \
             Per RFC 6749 §3.1.2.4 the request is not redirected.",
        );
    }
    let client_id = query.get("client_id").map(String::as_str).unwrap_or("");
    if client_id.is_empty() {
        return authorize_error_page(
            "Missing client_id",
            "The client_id parameter is required (RFC 6749 §4.1.1).",
        );
    }
    let response_type = query.get("response_type").map(String::as_str).unwrap_or("");
    let state = query.get("state").map(String::as_str);

    // PKCE (RFC 7636): challenge method defaults to "plain"; unknown methods
    // are rejected via the redirect error channel like any other bad request
    // parameter. Only meaningful for the code flow.
    let challenge = query
        .get("code_challenge")
        .cloned()
        .filter(|c| !c.is_empty());
    let method = query
        .get("code_challenge_method")
        .cloned()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "plain".to_string());
    if challenge.is_some() && !matches!(method.as_str(), "S256" | "plain") {
        return oauth_redirect(
            redirect_uri,
            &error_params("invalid_request", "Unknown code_challenge_method", state),
            false,
        );
    }

    match response_type {
        "code" | "token" => {}
        "" => {
            return oauth_redirect(
                redirect_uri,
                &error_params(
                    "invalid_request",
                    "Missing required parameter: response_type",
                    state,
                ),
                false,
            );
        }
        _ => {
            // §4.1.2.1: valid redirect_uri ⇒ the error must reach the client
            // through the redirect, not a bare 400.
            return oauth_redirect(
                redirect_uri,
                &error_params(
                    "unsupported_response_type",
                    "Only 'code' and 'token' are supported",
                    state,
                ),
                false,
            );
        }
    }

    consent_page(response_type, client_id, redirect_uri, &query, &method)
}

/// `POST /oauth2/authorize` — process the consent decision. The form carries
/// every authorize parameter through hidden fields; nothing is trusted from
/// the GET round-trip.
pub async fn oauth2_authorize_post_handler(
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let redirect_uri = form.get("redirect_uri").map(String::as_str).unwrap_or("");
    if !is_http_url(redirect_uri) {
        return authorize_error_page(
            "Invalid redirect_uri",
            "The redirect_uri field is missing or not an absolute http(s) URL.",
        );
    }
    let state = form.get("state").map(String::as_str);
    let response_type = form.get("response_type").map(String::as_str).unwrap_or("");
    let fragment = response_type == "token";

    // Same response_type validation as the GET leg: a consent POST for an
    // unsupported flow reports the error back to the client via redirect
    // (§4.1.2.1) instead of minting a code for an unknown flow.
    if !matches!(response_type, "code" | "token") {
        return oauth_redirect(
            redirect_uri,
            &error_params(
                if response_type.is_empty() {
                    "invalid_request"
                } else {
                    "unsupported_response_type"
                },
                if response_type.is_empty() {
                    "Missing required parameter: response_type"
                } else {
                    "Only 'code' and 'token' are supported"
                },
                state,
            ),
            false,
        );
    }

    let decision = form.get("decision").map(String::as_str).unwrap_or("");
    if decision != "approve" {
        // §4.1.2.1 / §4.2.2.1: denial (or anything but approval) is reported
        // back to the client — query for the code flow, fragment for implicit.
        return oauth_redirect(
            redirect_uri,
            &error_params(
                "access_denied",
                "The resource owner denied the authorization request",
                state,
            ),
            fragment,
        );
    }

    let email = form.get("email").map(String::as_str).unwrap_or("");
    if email.is_empty() {
        return authorize_error_page(
            "Missing email",
            "The consent form requires an email to act as the mock identity.",
        );
    }
    let client_id = form.get("client_id").cloned().unwrap_or_default();
    let scope = form.get("scope").cloned().filter(|s| !s.is_empty());

    if response_type == "token" {
        // Implicit flow (§4.2.2): access token in the fragment, never a
        // refresh token.
        let claims = BearerClaims::access(client_id, email.to_string(), scope.clone());
        let token = sign(&claims);
        let mut params = vec![
            ("access_token".to_string(), token),
            ("token_type".to_string(), "Bearer".to_string()),
            ("expires_in".to_string(), "3600".to_string()),
        ];
        if let Some(scope) = scope {
            params.push(("scope".to_string(), scope));
        }
        if let Some(st) = state_param(state) {
            params.push(st);
        }
        return oauth_redirect(redirect_uri, &params, true);
    }

    // Authorization code flow (§4.1.2): bind client_id + redirect_uri + PKCE
    // challenge into the one-time code.
    let mut claims = CodeClaims::new(
        client_id,
        redirect_uri.to_string(),
        state.map(str::to_string),
        email.to_string(),
    );
    claims.scope = scope;
    if let Some(challenge) = form
        .get("code_challenge")
        .cloned()
        .filter(|c| !c.is_empty())
    {
        claims.chal = Some(challenge);
        claims.chm = Some(
            form.get("code_challenge_method")
                .cloned()
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| "plain".to_string()),
        );
    }
    let code = sign(&claims);
    let mut params = vec![("code".to_string(), code)];
    if let Some(st) = state_param(state) {
        params.push(st);
    }
    oauth_redirect(redirect_uri, &params, false)
}

fn error_params(error: &str, description: &str, state: Option<&str>) -> Vec<(String, String)> {
    let mut params = vec![
        ("error".to_string(), error.to_string()),
        ("error_description".to_string(), description.to_string()),
    ];
    if let Some(st) = state_param(state) {
        params.push(st);
    }
    params
}

/// Standalone 400 page for failures that must not be redirected. Every
/// dynamic field is escaped; the fixed description strings stay ASCII.
fn authorize_error_page(title: &str, description: &str) -> HttpResponse {
    let card = format!(
        "<div class=\"rim\"><span>Authorization error</span><span class=\"pill pill-err\">400 &middot; not redirected</span></div>\n\
<div class=\"body\">\n\
<div><p class=\"err-title\">{title}</p><p class=\"err-desc\">{description}</p></div>\n\
<div class=\"notice notice-err\">{icon}<p>The request can&rsquo;t be safely sent back to the client, so it stops here. \
Fix the parameters in the authorize URL and try again.</p></div>\n\
</div>\n\
<div class=\"actions\">\n\
<a class=\"btn btn-outline\" href=\"/\">Home</a>\n\
<a class=\"btn btn-primary\" href=\"/oauth2\">OAuth 2.0 index</a>\n\
</div>",
        icon = icon("<circle cx=\"12\" cy=\"12\" r=\"10\"/><path d=\"M12 8v4\"/><path d=\"M12 16h.01\"/>"),
        title = html_escape(title),
        description = html_escape(description),
    );
    HttpResponse::BadRequest()
        .content_type("text/html; charset=utf-8")
        .insert_header(("X-Robots-Tag", "noindex"))
        .body(page_shell(
            "httpcan OAuth2 - error",
            "OAuth 2.0 mock server",
            &card,
            &[
                ("/oauth2", "About this mock"),
                (
                    "/.well-known/oauth-authorization-server",
                    "Discovery metadata",
                ),
            ],
        ))
}

/// The consent page: hidden fields round-trip every authorize parameter,
/// a mock notice marks this as a mock server, and every echoed field is
/// HTML-escaped (no template layer otherwise).
fn consent_page(
    response_type: &str,
    client_id: &str,
    redirect_uri: &str,
    query: &HashMap<String, String>,
    pkce_method: &str,
) -> HttpResponse {
    let scope = query.get("scope").map(String::as_str).unwrap_or("");
    let state = query.get("state").map(String::as_str).unwrap_or("");
    let challenge = query
        .get("code_challenge")
        .map(String::as_str)
        .unwrap_or("");
    let implicit = response_type == "token";

    let hidden = |name: &str, value: &str| {
        format!(
            "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
            name,
            html_escape(value)
        )
    };
    let hidden_fields = [
        hidden("response_type", response_type),
        hidden("client_id", client_id),
        hidden("redirect_uri", redirect_uri),
        hidden("state", state),
        hidden("scope", scope),
        hidden("code_challenge", challenge),
        hidden("code_challenge_method", pkce_method),
    ]
    .join("\n");

    // Client identity: first character as a monogram, redirect host as the
    // "where you'll land" hint (redirect_uri is already validated as a URL).
    let initial = client_id
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    let redirect_host = url::Url::parse(redirect_uri)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| u.port().map_or(h.to_string(), |p| format!("{h}:{p}")))
        })
        .unwrap_or_default();

    // Flow badges: grant type, PKCE status, deprecation for implicit.
    let mut badges = Vec::new();
    if implicit {
        badges.push("<span class=\"pill\">Implicit flow</span>".to_string());
        badges.push(
            "<span class=\"pill pill-warn\">Legacy &middot; removed in OAuth 2.1</span>"
                .to_string(),
        );
    } else {
        badges.push("<span class=\"pill\">Authorization code</span>".to_string());
        if challenge.is_empty() {
            badges.push("<span class=\"pill pill-warn\">No PKCE</span>".to_string());
        } else {
            badges.push(format!(
                "<span class=\"pill pill-ok\">PKCE &middot; {}</span>",
                html_escape(pkce_method)
            ));
        }
    }
    if state.is_empty() {
        badges.push("<span class=\"pill pill-warn\">No state</span>".to_string());
    }

    let scopes_html = if scope.trim().is_empty() {
        "<p class=\"empty\">No specific scopes requested.</p>".to_string()
    } else {
        let items: String = scope
            .split_whitespace()
            .map(|s| format!("<li>{}</li>", html_escape(s)))
            .collect();
        format!("<ul class=\"scopes\">{items}</ul>")
    };

    let dd = |v: &str| {
        if v.is_empty() {
            "<span class=\"none\">not sent</span>".to_string()
        } else {
            format!("<code>{}</code>", html_escape(v))
        }
    };
    let pkce_rows = if implicit {
        String::new()
    } else {
        format!(
            "<dt>code_challenge</dt><dd>{}</dd>\n<dt>code_challenge_method</dt><dd>{}</dd>\n",
            dd(challenge),
            if challenge.is_empty() {
                dd("")
            } else {
                dd(pkce_method)
            },
        )
    };
    let delivery = if implicit {
        "Approving sends an access token back in the redirect URL fragment."
    } else {
        "Approving sends a one-time authorization code back in the redirect URL."
    };

    let chevron = icon("<path d=\"m9 18 6-6-6-6\"/>");
    let check_icon = icon("<path d=\"M20 6 9 17l-5-5\"/>");
    let card = format!(
        "<div class=\"rim\"><span>Authorization request</span><span class=\"pill pill-warn\">Mock</span></div>\n\
<form method=\"POST\" action=\"/oauth2/authorize\">\n\
{hidden_fields}\n\
<div class=\"body\">\n\
<div class=\"client\"><div class=\"avatar\" aria-hidden=\"true\">{initial}</div>\
<div><h1><b>{client_id}</b> wants to access your account</h1>\
<p class=\"sub\">You&rsquo;ll be sent back to <b>{redirect_host}</b></p></div></div>\n\
<div class=\"badges\">{badges}</div>\n\
<div><p class=\"section-label\">Requested scopes</p>{scopes_html}</div>\n\
<div class=\"field\">\n<label for=\"email\">Sign in as</label>\n\
<input class=\"input\" type=\"email\" id=\"email\" name=\"email\" required autofocus autocomplete=\"email\" placeholder=\"user@example.com\">\n\
<p class=\"hint\">Any address works &mdash; it becomes the <code>sub</code> / <code>email</code> of the issued tokens.</p>\n\
</div>\n\
<details>\n<summary>{chevron}Request details</summary>\n<dl>\n\
<dt>response_type</dt><dd>{rt}</dd>\n\
<dt>client_id</dt><dd>{client_id_dd}</dd>\n\
<dt>redirect_uri</dt><dd>{redirect_dd}</dd>\n\
<dt>scope</dt><dd>{scope_dd}</dd>\n\
<dt>state</dt><dd>{state_dd}</dd>\n\
{pkce_rows}\
</dl>\n</details>\n\
<div class=\"notice\">{warn}<p><b>This is a mock consent screen.</b> No real account is involved and nothing is authorized &mdash; \
it exists to test OAuth 2.0 clients. {delivery} If you didn&rsquo;t expect this page, close it.</p></div>\n\
</div>\n\
<div class=\"actions\">\n\
<button class=\"btn btn-outline\" type=\"submit\" name=\"decision\" value=\"decline\" formnovalidate>Decline</button>\n\
<button class=\"btn btn-primary\" type=\"submit\" name=\"decision\" value=\"approve\">{check_icon}Approve</button>\n\
</div>\n\
</form>",
        initial = html_escape(&initial),
        client_id = html_escape(client_id),
        redirect_host = html_escape(&redirect_host),
        badges = badges.join(""),
        rt = dd(response_type),
        client_id_dd = dd(client_id),
        redirect_dd = dd(redirect_uri),
        scope_dd = dd(scope),
        state_dd = dd(state),
        warn = warn_icon(),
    );
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("X-Robots-Tag", "noindex"))
        .body(page_shell(
            "httpcan OAuth2 Consent",
            "OAuth 2.0 mock server",
            &card,
            &[
                ("/oauth2", "About this mock"),
                (
                    "/.well-known/oauth-authorization-server",
                    "Discovery metadata",
                ),
            ],
        ))
}
