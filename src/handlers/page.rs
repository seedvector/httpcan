//! Shared shell for httpcan's server-rendered single-card HTML pages (the
//! OAuth 2.0 consent/error pages and the `/redirect-to` browser
//! interstitial): brand stylesheet, inline icons, document wrapper.

/// Stylesheet for the single-card pages, in the homepage brand (paper / tin /
/// string-blue palette, Schibsted Grotesk + Martian Mono served from
/// `/fonts/`). No JS and no third-party assets: `<details>` handles
/// collapsible sections. Lucide icons are inlined as stroke SVGs.
pub(crate) const PAGE_CSS: &str = r#"
:root{color-scheme:light dark;--paper:#F5F7F8;--raised:#FFFFFF;--ink:#17202B;--muted:#56616D;
--tin:#DCE2E6;--tin-edge:#A9B4BD;--string:#1E88E5;--string-hover:#1565C0;--on-string:#FFFFFF;
--warn-bg:#FFF6E0;--warn-edge:#E8C77A;--warn-ink:#7A4B00;--err:#C62828;--err-bg:#FDECEC;--err-edge:#EBA9A9;
--sans:"Schibsted Grotesk",ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;
--mono:"Martian Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
@media (prefers-color-scheme:dark){:root{--paper:#161D26;--raised:#1C2530;--ink:#E8EDF1;--muted:#9AA6B2;
--tin:#26303C;--tin-edge:#45525F;--string-hover:#42A5F5;--warn-bg:#2B2414;--warn-edge:#6B5523;--warn-ink:#F2C766;
--err:#FF8A80;--err-bg:#2E1B1D;--err-edge:#6E3236}}
*,*::before,*::after{box-sizing:border-box}
body{margin:0;min-height:100svh;display:flex;flex-direction:column;align-items:center;justify-content:center;
gap:1rem;padding:2rem 1rem;background:var(--paper);color:var(--ink);font:400 .9375rem/1.55 var(--sans);
-webkit-font-smoothing:antialiased;-webkit-text-size-adjust:100%}
a{color:inherit;text-decoration:underline;text-decoration-color:var(--tin-edge);text-underline-offset:.2em}
a:hover{text-decoration-color:var(--string)}
:focus-visible{outline:2px solid var(--string);outline-offset:2px;border-radius:3px}
code{font-family:var(--mono);font-stretch:87.5%;font-size:.8125rem}
svg{flex:none}
.top{width:100%;max-width:30rem;display:flex;align-items:baseline;justify-content:space-between;gap:1rem}
.wordmark{font-family:var(--mono);font-weight:800;font-stretch:112.5%;font-size:1.0625rem;letter-spacing:-.02em;text-decoration:none}
.top-note{font-size:.8125rem;color:var(--muted)}
.can{width:100%;max-width:30rem;background:var(--raised);border:1.5px solid var(--tin-edge);border-radius:14px;overflow:hidden}
.rim{display:flex;justify-content:space-between;align-items:center;gap:1rem;padding:.6rem 1.25rem;
font-size:.875rem;font-weight:700;border-bottom:1.5px solid var(--tin-edge);background-color:var(--tin);
background-image:repeating-linear-gradient(90deg,transparent 0 4px,rgba(127,127,127,.16) 4px 5px)}
.pill{display:inline-flex;align-items:center;gap:.3rem;padding:.0625rem .5rem;border-radius:999px;
font-family:var(--mono);font-stretch:87.5%;font-size:.6875rem;font-weight:600;letter-spacing:.02em;
text-transform:uppercase;border:1px solid var(--tin-edge);background:var(--raised);color:var(--muted);white-space:nowrap}
.pill-warn{background:var(--warn-bg);border-color:var(--warn-edge);color:var(--warn-ink)}
.pill-err{background:var(--err-bg);border-color:var(--err-edge);color:var(--err)}
.pill-ok{border-color:var(--string);color:var(--string)}
.body{display:flex;flex-direction:column;gap:1.25rem;padding:1.5rem 1.25rem}
.client{display:flex;align-items:center;gap:.875rem}
.avatar{flex:none;display:grid;place-items:center;width:3rem;height:3rem;border-radius:10px;
background:var(--string);color:var(--on-string);font:800 1.375rem/1 var(--mono)}
.client h1{margin:0;font-size:1.1875rem;font-weight:600;line-height:1.3;letter-spacing:-.01em;overflow-wrap:anywhere}
.client h1 b{font-weight:800}
.sub{margin:.125rem 0 0;font-size:.875rem;color:var(--muted);overflow-wrap:anywhere}
.badges{display:flex;flex-wrap:wrap;gap:.375rem}
.section-label{margin:0 0 .5rem;font-size:.75rem;font-weight:700;letter-spacing:.06em;text-transform:uppercase;color:var(--muted)}
.scopes{display:flex;flex-wrap:wrap;gap:.375rem;margin:0;padding:0;list-style:none}
.scopes li{padding:.25rem .625rem;border:1px solid var(--tin-edge);border-radius:6px;background:var(--paper);
font-family:var(--mono);font-stretch:87.5%;font-size:.8125rem;overflow-wrap:anywhere}
.empty{margin:0;font-size:.875rem;color:var(--muted)}
.notice{display:flex;gap:.625rem;padding:.75rem .875rem;border:1px solid var(--warn-edge);border-radius:8px;
background:var(--warn-bg);color:var(--warn-ink);font-size:.8125rem;line-height:1.5}
.notice svg{margin-top:.125rem}
.notice p{margin:0}
.notice-err{border-color:var(--err-edge);background:var(--err-bg);color:var(--err)}
.field{display:flex;flex-direction:column;gap:.375rem}
.field label{font-size:.875rem;font-weight:600}
.input{height:2.625rem;width:100%;padding:0 .875rem;border:1.5px solid var(--tin-edge);border-radius:8px;
background:var(--paper);color:inherit;font:inherit;font-size:.9375rem}
.input:focus{border-color:var(--string);outline:none;box-shadow:0 0 0 3px rgb(30 136 229/.2)}
.hint{margin:0;font-size:.8125rem;color:var(--muted)}
details{border:1px solid var(--tin);border-radius:8px}
summary{cursor:pointer;padding:.625rem .875rem;font-size:.875rem;font-weight:600;color:var(--muted);list-style:none;
display:flex;align-items:center;gap:.5rem}
summary::-webkit-details-marker{display:none}
summary svg{transition:transform .15s}
details[open] summary svg{transform:rotate(90deg)}
summary:hover{color:var(--ink)}
dl{margin:0;padding:0 .875rem .75rem;display:grid;grid-template-columns:auto 1fr;gap:.5rem 1rem;font-size:.8125rem}
dt{color:var(--muted)}
dd{margin:0;min-width:0;overflow-wrap:anywhere}
dd .none{color:var(--muted);font-style:italic;font-family:var(--sans)}
.actions{display:grid;grid-template-columns:1fr 1fr;gap:.625rem;padding:1rem 1.25rem;border-top:1.5px solid var(--tin)}
.btn{display:inline-flex;align-items:center;justify-content:center;gap:.5rem;height:2.625rem;padding:0 1rem;
border-radius:8px;font:600 .9375rem/1 var(--sans);border:1.5px solid transparent;cursor:pointer;text-decoration:none;color:inherit}
.btn-primary{background:var(--string);color:var(--on-string)}
.btn-primary:hover{background:var(--string-hover)}
.btn-outline{border-color:var(--tin-edge);background:var(--raised)}
.btn-outline:hover{background:var(--tin)}
.err-title{margin:0;font-size:1.25rem;font-weight:800;letter-spacing:-.01em}
.err-desc{margin:.375rem 0 0}
.foot{width:100%;max-width:30rem;display:flex;justify-content:center;gap:1.25rem;font-size:.8125rem;color:var(--muted)}
.dest{display:flex;flex-direction:column;gap:.375rem;padding:.875rem 1rem;border:1.5px solid var(--tin-edge);
border-radius:10px;background:var(--paper)}
.dest-host{display:flex;align-items:center;gap:.5rem;font-size:1.0625rem;font-weight:700;overflow-wrap:anywhere}
.dest-host svg{color:var(--muted)}
.dest-url{margin:0;font-family:var(--mono);font-stretch:87.5%;font-size:.8125rem;color:var(--muted);overflow-wrap:anywhere}
.lead{margin:0}
.btn[aria-disabled=true]{opacity:.55;cursor:not-allowed;pointer-events:none}
@media (max-width:26rem){.body{padding:1.25rem 1rem}.actions{padding:1rem}.rim{padding:.6rem 1rem}}
"#;

pub(crate) const ICON_ATTRS: &str = "xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"";

pub(crate) fn icon(paths: &str) -> String {
    format!("<svg {ICON_ATTRS}>{paths}</svg>")
}

pub(crate) fn warn_icon() -> String {
    icon("<path d=\"m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3\"/><path d=\"M12 9v4\"/><path d=\"M12 17h.01\"/>")
}

/// HTML-escape every dynamic field rendered into these pages. There is no
/// template layer, so this is the only XSS defense.
pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// HTML document shell: wordmark strip, one "can" card, footer links.
/// `footer` is `(href, label)` pairs; all text is escaped here, while `card`
/// is trusted markup the caller has already escaped.
pub(crate) fn page_shell(
    title: &str,
    top_note: &str,
    card: &str,
    footer: &[(&str, &str)],
) -> String {
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
    let footer: String = footer
        .iter()
        .map(|(href, label)| {
            format!(
                "<a href=\"{}\">{}</a>",
                html_escape(href),
                html_escape(label)
            )
        })
        .collect();
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<meta name=\"robots\" content=\"noindex\">\n\
<title>{title}</title>\n<link rel=\"icon\" type=\"image/png\" href=\"/favicon.png\">\n\
<style>{fonts}{PAGE_CSS}</style>\n</head>\n<body>\n\
<header class=\"top\"><a class=\"wordmark\" href=\"/\">HTTPCan</a><span class=\"top-note\">{top_note}</span></header>\n\
<main class=\"can\">\n{card}\n</main>\n\
<footer class=\"foot\">{footer}</footer>\n\
</body>\n</html>\n",
        title = html_escape(title),
        top_note = html_escape(top_note),
        fonts = crate::handlers::root::font_faces(version),
    )
}
