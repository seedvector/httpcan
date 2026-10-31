//! Auth flows: basic, hidden-basic, bearer, JWT bearer, digest

use super::*;

#[actix_web::test]
async fn basic_auth_accepts_post() {
    // httpbin #365/#607: basic-auth must accept POST credentials.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q=")) // user:passwd
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn digest_auth_sha_512_256_succeeds() {
    // httpbin #697: SHA-512-256 (RFC 7616) digest auth end-to-end. curl has
    // no SHA-512-256 client, so the digest response is computed here.
    let app = test::init_service(create_app(cfg())).await;
    let uri = "/digest-auth/auth/user/passwd/SHA-512-256";

    // 1) Challenge: pull nonce/realm/opaque from the 401 WWW-Authenticate.
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("www-authenticate")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let field = |k: &str| -> String {
        let pat = format!("{k}=\"");
        let start = challenge.find(&pat).unwrap() + pat.len();
        let end = challenge[start..].find('"').unwrap();
        challenge[start..start + end].to_string()
    };
    let realm = field("realm");
    let nonce = field("nonce");
    let opaque = field("opaque");

    // 2) Compute the digest response (qop=auth).
    let hex = |data: &[u8]| -> String {
        crate::handlers::auth::digest_hash_hex("SHA-512-256", data).unwrap()
    };
    let ha1 = hex(format!("user:{realm}:passwd").as_bytes());
    let ha2 = hex(format!("GET:{uri}").as_bytes());
    let nc = "00000001";
    let cnonce = "0a4f113b";
    let response = hex(format!("{ha1}:{nonce}:{nc}:{cnonce}:auth:{ha2}").as_bytes());
    let auth = format!(
        r#"Digest username="user", realm="{realm}", nonce="{nonce}", uri="{uri}", qop=auth, nc={nc}, cnonce="{cnonce}", response="{response}", opaque="{opaque}", algorithm=SHA-512-256"#
    );

    // 3) Replay with credentials -> 200, authenticated.
    let req = test::TestRequest::get()
        .uri(uri)
        .insert_header(("Authorization", auth))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("algorithm").and_then(|x| x.as_str()),
        Some("SHA-512-256")
    );
}

#[actix_web::test]
async fn digest_auth_omits_qop_rfc2069() {
    // httpbin #592: `/digest-auth/none/...` must issue a challenge with NO
    // qop directive (RFC 2069 legacy mode) and accept an RFC 2069 response
    // computed as H(HA1:nonce:HA2) — no nc/cnonce/qop.
    let app = test::init_service(create_app(cfg())).await;
    let uri = "/digest-auth/none/user/passwd";

    // 1) Challenge: 401, and qop must be ABSENT.
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("www-authenticate")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        !challenge.contains("qop="),
        "RFC 2069 challenge must omit qop, got: {challenge}"
    );
    let field = |k: &str| -> String {
        let pat = format!("{k}=\"");
        let start = challenge.find(&pat).unwrap() + pat.len();
        let end = challenge[start..].find('"').unwrap();
        challenge[start..start + end].to_string()
    };
    let realm = field("realm");
    let nonce = field("nonce");
    let opaque = field("opaque");

    // 2) RFC 2069 response = MD5(MD5(user:realm:passwd):nonce:MD5(GET:uri)).
    let ha1 = format!("{:x}", md5::compute(format!("user:{realm}:passwd")));
    let ha2 = format!("{:x}", md5::compute(format!("GET:{uri}")));
    let response = format!("{:x}", md5::compute(format!("{ha1}:{nonce}:{ha2}")));
    let auth = format!(
        r#"Digest username="user", realm="{realm}", nonce="{nonce}", uri="{uri}", response="{response}", opaque="{opaque}""#
    );

    // 3) Replay with credentials -> 200, authenticated.
    let req = test::TestRequest::get()
        .uri(uri)
        .insert_header(("Authorization", auth))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
}

/// /basic-auth/user/passwd with correct Basic creds returns 200, authenticated=true, user="user".
#[actix_web::test]
async fn auth_basic_correct_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q=")) // user:passwd
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(v.get("user").and_then(|x| x.as_str()), Some("user"));
}

/// /basic-auth/user/passwd with no Authorization header returns 401 + WWW-Authenticate challenge.
#[actix_web::test]
async fn auth_basic_no_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user/passwd")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let www = resp
        .headers()
        .get("WWW-Authenticate")
        .expect("WWW-Authenticate present");
    assert!(www.to_str().unwrap().starts_with("Basic realm="));
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /basic-auth/user/passwd with wrong creds returns 401 + WWW-Authenticate challenge.
#[actix_web::test]
async fn auth_basic_wrong_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic d3Jvbmc6d3Jvbmc=")) // wrong:wrong
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("WWW-Authenticate"));
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /basic-auth/user (username-only route) with Basic dXNlcjo= (user:) → 200, password must be empty.
#[actix_web::test]
async fn auth_basic_user_only_empty_password() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/basic-auth/user")
        .insert_header(("Authorization", "Basic dXNlcjo=")) // user:
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(v.get("user").and_then(|x| x.as_str()), Some("user"));
}

/// /hidden-basic-auth/user/passwd with correct creds → 200, authenticated=true.
#[actix_web::test]
async fn auth_hidden_basic_correct_creds() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q=")) // user:passwd
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(v.get("user").and_then(|x| x.as_str()), Some("user"));
}

/// /hidden-basic-auth with no creds returns 404 (hidden variant) and NO WWW-Authenticate header.
#[actix_web::test]
async fn auth_hidden_basic_no_creds_is_404() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user/passwd")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // Hidden variant must NOT advertise a WWW-Authenticate challenge (it hides the endpoint).
    assert!(resp.headers().get("WWW-Authenticate").is_none());
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /hidden-basic-auth with wrong creds returns 404 and NO WWW-Authenticate header.
#[actix_web::test]
async fn auth_hidden_basic_wrong_creds_is_404() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user/passwd")
        .insert_header(("Authorization", "Basic d3Jvbmc6d3Jvbmc=")) // wrong:wrong
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert!(resp.headers().get("WWW-Authenticate").is_none());
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /hidden-basic-auth/{user} (user-only variant): empty password matches → 200;
/// otherwise the hidden variant stays hidden and returns 404 (no challenge).
#[actix_web::test]
async fn auth_hidden_basic_user_only_succeeds_and_hides_failure() {
    let app = test::init_service(create_app(cfg())).await;
    // Correct: username matches, password empty (Basic "user:" = dXNlcjo=).
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user")
        .insert_header(("Authorization", "Basic dXNlcjo="))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    // No creds → stays hidden (404, no WWW-Authenticate).
    let req = test::TestRequest::get()
        .uri("/hidden-basic-auth/user")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert!(resp.headers().get("WWW-Authenticate").is_none());
}

/// /bearer with a valid Bearer token → 200, authenticated=true, token echoed back.
#[actix_web::test]
async fn auth_bearer_valid_token() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/bearer")
        .insert_header(("Authorization", "Bearer my-secret-token"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("token").and_then(|x| x.as_str()),
        Some("my-secret-token")
    );
}

/// /bearer with no Authorization header → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_bearer_no_header() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/bearer").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let www = resp
        .headers()
        .get("WWW-Authenticate")
        .expect("WWW-Authenticate present");
    assert_eq!(www.to_str().unwrap(), "Bearer");
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// /bearer with a non-Bearer (Basic) Authorization header → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_bearer_invalid_scheme() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/bearer")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q="))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
}

/// /jwt-bearer with a structurally valid JWT (no exp claim) → 200, decodes header + payload claims.
#[actix_web::test]
async fn auth_jwt_valid_decodes_claims() {
    // Canonical jwt.io example token (HS256). Signature is NOT verified by the
    // handler (insecure_decode), so a well-formed token always decodes. No `exp`
    // claim ⇒ expiration status "not_present" ⇒ treated as valid (200).
    let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
                 eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.\
                 SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/jwt-bearer")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(v.get("authenticated").and_then(|x| x.as_bool()), Some(true));
    // raw token echoed verbatim
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("raw"))
            .and_then(|x| x.as_str()),
        Some(token)
    );
    // header decoded
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("header"))
            .and_then(|h| h.get("alg"))
            .and_then(|x| x.as_str()),
        Some("HS256")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("header"))
            .and_then(|h| h.get("typ"))
            .and_then(|x| x.as_str()),
        Some("JWT")
    );
    // payload claims decoded (raw numeric iat retained)
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("sub"))
            .and_then(|x| x.as_str()),
        Some("1234567890")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("name"))
            .and_then(|x| x.as_str()),
        Some("John Doe")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("iat"))
            .and_then(|x| x.as_i64()),
        Some(1516239022)
    );
    // payloadFormatted carries a human-readable iat timestamp string
    assert!(
        v.get("token")
            .and_then(|t| t.get("payloadFormatted"))
            .and_then(|p| p.get("iat"))
            .map(|x| x.is_string())
            .unwrap_or(false),
        "payloadFormatted.iat must be a formatted timestamp string"
    );
    // validation status reflects structure + (missing) expiration
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("validationStatus"))
            .and_then(|s| s.get("structure"))
            .and_then(|x| x.as_str()),
        Some("valid")
    );
    assert_eq!(
        v.get("token")
            .and_then(|t| t.get("validationStatus"))
            .and_then(|s| s.get("expiration"))
            .and_then(|x| x.as_str()),
        Some("not_present")
    );
}

/// /jwt-bearer with a malformed token (wrong number of segments) → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_jwt_malformed_is_401() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/jwt-bearer")
        .insert_header(("Authorization", "Bearer not-a-real-jwt")) // single segment
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
    assert!(v.get("error").is_some());
}

/// /jwt-bearer with no Authorization header → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_jwt_missing_header_is_401() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/jwt-bearer").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
    assert_eq!(
        v.get("error").and_then(|x| x.as_str()),
        Some("Missing Authorization header")
    );
}

/// /jwt-bearer with a non-Bearer Authorization scheme → 401 + WWW-Authenticate: Bearer.
#[actix_web::test]
async fn auth_jwt_wrong_scheme_is_401() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/jwt-bearer")
        .insert_header(("Authorization", "Basic dXNlcjpwYXNzd2Q="))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("WWW-Authenticate")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer"
    );
}

/// /digest-auth/auth/user/passwd with no creds → 401 with a Digest challenge containing
/// realm, nonce, qop="auth", and algorithm="MD5".
#[actix_web::test]
async fn auth_digest_challenge_shape() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/digest-auth/auth/user/passwd")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("WWW-Authenticate")
        .expect("WWW-Authenticate present")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        challenge.starts_with("Digest "),
        "challenge must be Digest scheme: {}",
        challenge
    );
    assert!(
        challenge.contains("realm=\"httpcan@"),
        "realm must be prefixed with httpcan@: {}",
        challenge
    );
    assert!(
        challenge.contains("nonce=\""),
        "missing nonce: {}",
        challenge
    );
    assert!(
        challenge.contains("opaque=\""),
        "missing opaque: {}",
        challenge
    );
    assert!(
        challenge.contains("qop=\"auth\""),
        "missing/incorrect qop directive: {}",
        challenge
    );
    assert!(
        challenge.contains("algorithm=\"MD5\""),
        "missing/incorrect algorithm: {}",
        challenge
    );
    let v: serde_json::Value =
        serde_json::from_slice(&test::read_body(resp).await).expect("JSON body");
    assert_eq!(
        v.get("authenticated").and_then(|x| x.as_bool()),
        Some(false)
    );
}

/// GET /digest-auth/{qop}/{user}/{passwd}/{algorithm}/{stale_after}
/// (digest_auth_full_handler) issues a 401 Digest challenge AND sets the
/// stale_after cookie — the behavior unique to the 5-segment route.
#[actix_web::test]
async fn digest_auth_full_five_segment_sets_stale_after_cookie() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/digest-auth/auth/user/passwd/MD5/never")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("www-authenticate")
        .expect("Digest challenge")
        .to_str()
        .unwrap();
    assert!(
        challenge.starts_with("Digest"),
        "challenge must be Digest scheme: {challenge}"
    );
    // The 5-segment route is the only one that plants the stale_after cookie.
    let set_cookie = resp
        .headers()
        .get_all("set-cookie")
        .map(|v| v.to_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("; ");
    assert!(
        set_cookie.contains("stale_after=never"),
        "5-segment route must set stale_after cookie: {set_cookie}"
    );
}
