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

use super::{
    html_escape, is_http_url, oauth_redirect, sign, state_param, BearerClaims, CodeClaims,
};
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

/// shadcn/ui design system, ported to plain CSS for the server-rendered
/// OAuth pages (no JS, no external assets): zinc semantic tokens with a
/// `prefers-color-scheme` dark theme, and the Card / Button / Input / Alert /
/// Table component shapes. Lucide icons are inlined as stroke SVGs.
const PAGE_CSS: &str = r#"
:root{--radius:.5rem;--background:0 0% 100%;--foreground:240 10% 3.9%;--card:0 0% 100%;
--card-foreground:240 10% 3.9%;--primary:240 5.9% 10%;--primary-foreground:0 0% 98%;
--muted:240 4.8% 95.9%;--muted-foreground:240 3.8% 46.1%;--accent:240 4.8% 95.9%;
--accent-foreground:240 5.9% 10%;--destructive:0 84.2% 60.2%;--border:240 5.9% 90%;
--input:240 5.9% 90%;--ring:240 10% 3.9%;--warning:32 95% 44%;--warning-muted:48 96% 89%}
@media (prefers-color-scheme:dark){:root{--background:240 10% 3.9%;--foreground:0 0% 98%;
--card:240 10% 3.9%;--card-foreground:0 0% 98%;--primary:0 0% 98%;--primary-foreground:240 5.9% 10%;
--muted:240 3.7% 15.9%;--muted-foreground:240 5% 64.9%;--accent:240 3.7% 15.9%;
--accent-foreground:0 0% 98%;--destructive:0 72% 51%;--border:240 3.7% 15.9%;
--input:240 3.7% 15.9%;--ring:240 4.9% 83.9%;--warning:46 96% 65%;--warning-muted:29 60% 14%}}
*{box-sizing:border-box}
body{margin:0;min-height:100svh;display:flex;align-items:center;justify-content:center;
padding:1.5rem;background:hsl(var(--background));color:hsl(var(--foreground));
font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;
-webkit-font-smoothing:antialiased;font-size:14px;line-height:1.43}
a{color:inherit}
.card{width:100%;max-width:28rem;border:1px solid hsl(var(--border));border-radius:.75rem;
background:hsl(var(--card));color:hsl(var(--card-foreground));box-shadow:0 1px 2px 0 rgb(0 0 0/.05)}
.card-header{display:flex;flex-direction:column;gap:.375rem;padding:1.5rem}
.card-title{font-size:1.125rem;font-weight:600;letter-spacing:-.02em;line-height:1.3}
.card-description{font-size:.875rem;color:hsl(var(--muted-foreground))}
.card-content{display:flex;flex-direction:column;gap:1rem;padding:0 1.5rem 1.5rem}
.card-footer{display:flex;align-items:center;gap:.5rem;padding:0 1.5rem 1.5rem;flex-wrap:wrap}
.alert{display:flex;gap:.625rem;border:1px solid hsl(var(--border));border-radius:.5rem;
padding:.75rem;background:hsl(var(--muted));font-size:.875rem}
.alert svg{flex:none;margin-top:.125rem}
.alert-warning{background:hsl(var(--warning-muted));border-color:hsl(var(--warning)/.35);color:hsl(var(--warning))}
.alert-warning .alert-body{color:inherit}
.alert-body b{color:hsl(var(--warning))}
.alert-destructive{border-color:hsl(var(--destructive)/.5);text-align:left}
.alert-destructive b{color:hsl(var(--destructive))}
.btn{display:inline-flex;align-items:center;justify-content:center;gap:.5rem;height:2.25rem;
padding:0 1rem;border-radius:calc(var(--radius) - .125rem);font-size:.875rem;font-weight:500;
border:1px solid transparent;cursor:pointer;text-decoration:none;color:inherit}
.btn:focus-visible,.input:focus-visible{outline:2px solid hsl(var(--ring));outline-offset:2px}
.btn-primary{background:hsl(var(--primary));color:hsl(var(--primary-foreground))}
.btn-primary:hover{opacity:.9}
.btn-outline{border-color:hsl(var(--input));background:hsl(var(--card))}
.btn-outline:hover{background:hsl(var(--accent))}
.input{height:2.25rem;width:100%;border:1px solid hsl(var(--input));border-radius:calc(var(--radius) - .125rem);
padding:0 .75rem;font-size:.875rem;background:transparent;color:inherit;font-family:inherit}
.field{display:flex;flex-direction:column;gap:.375rem}
.label{font-size:.875rem;font-weight:500}
.field-desc{margin:0;font-size:.8125rem;color:hsl(var(--muted-foreground))}
.data{width:100%;border-collapse:collapse;font-size:.875rem}
.data th{text-align:left;font-weight:500;color:hsl(var(--muted-foreground));
padding:.5rem .75rem .5rem 0;white-space:nowrap;vertical-align:baseline}
.data td{padding:.5rem 0}
.data tr+tr th,.data tr+tr td{border-top:1px solid hsl(var(--border))}
code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.8125rem;
background:hsl(var(--muted));padding:.125rem .375rem;border-radius:.25rem;word-break:break-all}
.ask{margin:0;font-weight:500}
.footer-link{margin-left:auto;font-size:.875rem;color:hsl(var(--muted-foreground))}
.footer-link:hover{color:hsl(var(--foreground))}
"#;

/// HTML document shell wrapping one centered card.
fn page_shell(title: &str, card: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{title}</title>\n<style>{PAGE_CSS}</style>\n</head>\n<body>\n{card}\n</body>\n</html>\n",
        title = html_escape(title),
    )
}

/// Standalone 400 page for failures that must not be redirected. Every
/// dynamic field is escaped; the fixed description strings stay ASCII.
fn authorize_error_page(title: &str, description: &str) -> HttpResponse {
    let card = format!(
        "<div class=\"card\">\n\
<div class=\"card-header\">\n<div class=\"card-title\">Authorization error</div>\n\
<div class=\"card-description\">httpcan OAuth 2.0 mock</div>\n</div>\n\
<div class=\"card-content\">\n\
<div class=\"alert alert-warning\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3\"/><path d=\"M12 9v4\"/><path d=\"M12 17h.01\"/></svg><div class=\"alert-body\">This is a mock OAuth2 service. It does not authenticate anyone.</div></div>\n\
<div>\n<b>{title}</b>\n<p>{description}</p>\n</div>\n\
</div>\n\
<div class=\"card-footer\">\n<a class=\"btn btn-outline\" href=\"/oauth2\">Back to the /oauth2 index</a></div>\n\
</div>",
        title = html_escape(title),
        description = html_escape(description),
    );
    HttpResponse::BadRequest()
        .content_type("text/html; charset=utf-8")
        .insert_header(("X-Robots-Tag", "noindex"))
        .body(page_shell("httpcan OAuth2 - error", &card))
}

/// The consent page: hidden fields round-trip every authorize parameter,
/// the mock warning banner marks this as a mock server, and every
/// echoed field is HTML-escaped (no template layer otherwise).
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
    let flow = if response_type == "token" {
        "implicit (response_type=token - legacy, removed in OAuth 2.1)"
    } else {
        "authorization code (response_type=code)"
    };
    let hidden = |name: &str, value: &str| {
        format!(
            "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
            name,
            html_escape(value)
        )
    };
    let scope_display = if scope.is_empty() {
        "-".to_string()
    } else {
        html_escape(scope)
    };
    let check_icon = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M20 6 9 17l-5-5\"/></svg>";
    let x_icon = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M18 6 6 18\"/><path d=\"m6 6 12 12\"/></svg>";
    let warn_icon = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3\"/><path d=\"M12 9v4\"/><path d=\"M12 17h.01\"/></svg>";
    let card = format!(
        "<div class=\"card\">\n\
<div class=\"card-header\">\n<div class=\"card-title\">Authorization request</div>\n\
<div class=\"card-description\">httpcan OAuth 2.0 mock consent</div>\n</div>\n\
<div class=\"card-content\">\n\
<div class=\"alert alert-warning\">{warn_icon}<div class=\"alert-body\"><b>This is a mock consent page.</b> \
It does not actually authorize the application - it exists to demonstrate and test the OAuth2 flow. \
If you do not understand any of that, close this page immediately.</div></div>\n\
<p class=\"ask\">An application is requesting access via the <code>{flow}</code> flow.</p>\n\
<table class=\"data\">\n\
<tr><th>Client ID</th><td><code>{client_id}</code></td></tr>\n\
<tr><th>Scope</th><td><code>{scope_display}</code></td></tr>\n\
<tr><th>Redirect after approval</th><td><code>{redirect_uri}</code></td></tr>\n\
</table>\n\
</div>\n\
<form method=\"POST\" action=\"/oauth2/authorize\">\n\
{hidden_response_type}\n{hidden_client_id}\n{hidden_redirect_uri}\n{hidden_state}\n{hidden_scope}\n{hidden_challenge}\n{hidden_method}\n\
<div class=\"card-content\">\n\
<div class=\"field\">\n<label class=\"label\" for=\"email\">Email to &ldquo;sign in&rdquo; as</label>\n\
<input class=\"input\" type=\"email\" id=\"email\" name=\"email\" required autofocus placeholder=\"user@example.com\">\n\
<p class=\"field-desc\">Any address works - it becomes the mock identity (sub/email) in the issued tokens.</p>\n\
</div>\n\
<p class=\"ask\">Do you want to authorize this application?</p>\n\
</div>\n\
<div class=\"card-footer\">\n\
<button class=\"btn btn-primary\" type=\"submit\" name=\"decision\" value=\"approve\">{check_icon}Approve</button>\n\
<button class=\"btn btn-outline\" type=\"submit\" name=\"decision\" value=\"decline\">{x_icon}Decline</button>\n\
<a class=\"footer-link\" href=\"/oauth2\">About this mock</a>\n\
</div>\n\
</form>\n\
</div>",
        warn_icon = warn_icon,
        check_icon = check_icon,
        x_icon = x_icon,
        flow = html_escape(flow),
        hidden_response_type = hidden("response_type", response_type),
        hidden_client_id = hidden("client_id", client_id),
        hidden_redirect_uri = hidden("redirect_uri", redirect_uri),
        hidden_state = hidden("state", state),
        hidden_scope = hidden("scope", scope),
        hidden_challenge = hidden("code_challenge", challenge),
        hidden_method = hidden("code_challenge_method", pkce_method),
        client_id = html_escape(client_id),
        scope_display = scope_display,
        redirect_uri = html_escape(redirect_uri),
    );
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("X-Robots-Tag", "noindex"))
        .body(page_shell("httpcan OAuth2 Consent", &card))
}
