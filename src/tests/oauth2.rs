//! OAuth 2.0 mock (RFC 6749 four grants + PKCE)

use super::*;

fn oauth2_location<B>(resp: &actix_web::dev::ServiceResponse<B>) -> String {
    resp.headers()
        .get("Location")
        .expect("redirect carries Location")
        .to_str()
        .unwrap()
        .to_string()
}

fn oauth2_query_param(location: &str, key: &str) -> Option<String> {
    url::Url::parse(location)
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.to_string())
}

fn oauth2_fragment_param(location: &str, key: &str) -> Option<String> {
    let fragment = url::Url::parse(location).ok()?.fragment()?.to_string();
    fragment
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}

/// POST the consent decision; returns the response and redirect Location.
async fn oauth2_decide(
    payload: &str,
) -> (
    actix_web::dev::ServiceResponse<actix_web::body::EitherBody<actix_web::body::BoxBody>>,
    String,
) {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/oauth2/authorize")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload(payload.to_string())
        .to_request();
    let resp = test::call_service(&app, req).await;
    let location = oauth2_location(&resp);
    (resp, location)
}

/// POST the token endpoint.
async fn oauth2_token(
    payload: &str,
) -> actix_web::dev::ServiceResponse<actix_web::body::EitherBody<actix_web::body::BoxBody>> {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::post()
        .uri("/oauth2/token")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload(payload.to_string())
        .to_request();
    test::call_service(&app, req).await
}

/// HMAC-SHA256 correctness pinned to the RFC 4231 test vector (case 1).
#[actix_web::test]
async fn oauth2_hmac_rfc4231_vector() {
    let key = vec![0x0bu8; 20];
    let mac = crate::handlers::oauth2::hmac_sha256(&key, b"Hi There");
    let expected: [u8; 32] = [
        0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b, 0xf1,
        0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c, 0x2e, 0x32,
        0xcf, 0xf7,
    ];
    assert_eq!(mac, expected.to_vec());
}

#[actix_web::test]
async fn oauth2_index_and_discovery_metadata() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/oauth2").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        v["grant_types_supported"].as_array().unwrap().len(),
        5,
        "four token grants plus implicit"
    );

    let req = test::TestRequest::get()
        .uri("/.well-known/oauth-authorization-server")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["issuer"], "http://localhost:8080");
    assert!(v["authorization_endpoint"]
        .as_str()
        .unwrap()
        .ends_with("/oauth2/authorize"));
    assert!(v["token_endpoint"]
        .as_str()
        .unwrap()
        .ends_with("/oauth2/token"));
    // RFC 8414 REQUIRED + OPTIONAL fields the implementation backs up.
    assert_eq!(
        v["response_types_supported"],
        serde_json::json!(["code", "token"])
    );
    assert_eq!(v["grant_types_supported"].as_array().unwrap().len(), 5);
    assert!(v["service_documentation"].as_str().unwrap().ends_with('/'));
}

#[actix_web::test]
async fn oauth2_consent_page_renders_and_escapes() {
    let app = test::init_service(create_app(cfg())).await;
    // client_id carries raw HTML: it must be escaped everywhere it renders.
    let req = test::TestRequest::get()
        .uri("/oauth2/authorize?response_type=code&client_id=%3Cscript%3Ealert(1)%3C%2Fscript%3E&redirect_uri=https://example.com/cb")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("X-Robots-Tag").unwrap(), "noindex");
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("OAuth2 Consent"));
    assert!(!body.contains("<script>"), "raw script must not survive");
    assert!(body.contains("&lt;script&gt;"));

    // Missing client_id must NOT be redirected (RFC 6749 §4.1.2.1).
    let req = test::TestRequest::get()
        .uri("/oauth2/authorize?response_type=code&redirect_uri=https://example.com/cb")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("Location").is_none());
}

#[actix_web::test]
async fn oauth2_invalid_redirect_uri_is_not_redirected() {
    let app = test::init_service(create_app(cfg())).await;
    // javascript: scheme must be refused without a redirect (§3.1.2.4).
    let req = test::TestRequest::get()
        .uri("/oauth2/authorize?response_type=code&client_id=cli&redirect_uri=javascript:alert(1)")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("Location").is_none());
}

#[actix_web::test]
async fn oauth2_unsupported_response_type_redirects_with_error() {
    // Valid redirect_uri ⇒ the error must reach the client via redirect
    // (§4.1.2.1), not a bare 400.
    let (resp, location) = oauth2_decide(
        "response_type=bogus&client_id=cli&redirect_uri=https://example.com/cb&state=xyz&email=u@example.com&decision=approve",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        oauth2_query_param(&location, "error").as_deref(),
        Some("unsupported_response_type")
    );
    assert_eq!(
        oauth2_query_param(&location, "state").as_deref(),
        Some("xyz")
    );
    assert!(oauth2_query_param(&location, "code").is_none());

    // Missing response_type is an invalid_request redirect.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/oauth2/authorize?client_id=cli&redirect_uri=https://example.com/cb")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = oauth2_location(&resp);
    assert_eq!(
        oauth2_query_param(&location, "error").as_deref(),
        Some("invalid_request")
    );
}

#[actix_web::test]
async fn oauth2_code_flow_happy_path() {
    let (resp, location) = oauth2_decide(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&state=xyz&scope=read&email=user@example.com&decision=approve",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let code = oauth2_query_param(&location, "code").expect("code in query");
    assert_eq!(
        oauth2_query_param(&location, "state").as_deref(),
        Some("xyz")
    );
    assert!(oauth2_fragment_param(&location, "access_token").is_none());

    let resp = oauth2_token(&format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=cli&client_secret=s3cr3t"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    // §5.1: no-store + no-cache on token responses.
    assert_eq!(resp.headers().get("Cache-Control").unwrap(), "no-store");
    assert_eq!(resp.headers().get("Pragma").unwrap(), "no-cache");
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["token_type"], "Bearer");
    assert_eq!(v["expires_in"], 3600);
    assert_eq!(v["scope"], "read");
    let access = v["access_token"].as_str().unwrap().to_string();
    assert!(v["refresh_token"].is_string());

    // userinfo accepts the access token and echoes the mock identity.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .insert_header(("Authorization", format!("Bearer {access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["sub"], "user@example.com");
    assert_eq!(v["email"], "user@example.com");
    assert_eq!(v["client_id"], "cli");
}

#[actix_web::test]
async fn oauth2_code_replay_rejected() {
    let (_, location) = oauth2_decide(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&email=u@example.com&decision=approve",
    )
    .await;
    let code = oauth2_query_param(&location, "code").unwrap();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=cli"
    );
    let resp = oauth2_token(&body).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // §4.1.2 MUST: a code used twice is denied.
    let resp = oauth2_token(&body).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "invalid_grant");
}

#[actix_web::test]
async fn oauth2_code_binding_mismatches_rejected() {
    let (_, location) = oauth2_decide(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&email=u@example.com&decision=approve",
    )
    .await;
    let code = oauth2_query_param(&location, "code").unwrap();

    for body in [
        format!("grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=other"),
        format!("grant_type=authorization_code&code={code}&redirect_uri=https://evil.example/cb&client_id=cli"),
    ] {
        let resp = oauth2_token(&body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
        assert_eq!(v["error"], "invalid_grant");
    }
}

#[actix_web::test]
async fn oauth2_expired_code_rejected() {
    // Craft a properly signed but expired code (RFC 6749 §4.1.2 code
    // lifetime) instead of waiting out the TTL.
    let mut claims = crate::handlers::oauth2::CodeClaims::new(
        "cli".into(),
        "https://example.com/cb".into(),
        None,
        "u@example.com".into(),
    );
    claims.iat = crate::handlers::oauth2::now_unix() - 700;
    claims.exp = crate::handlers::oauth2::now_unix() - 100;
    let code = crate::handlers::oauth2::sign(&claims);

    let resp = oauth2_token(&format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=cli"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "invalid_grant");
    assert!(v["error_description"].as_str().unwrap().contains("expired"));
}

#[actix_web::test]
async fn oauth2_pkce_s256_and_wrong_verifier() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    use sha2::{Digest, Sha256};

    let verifier = "correct-horse-battery-staple";
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let (_, location) = oauth2_decide(&format!(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&email=u@example.com&decision=approve&code_challenge={challenge}&code_challenge_method=S256"
    ))
    .await;
    let code = oauth2_query_param(&location, "code").unwrap();

    // Wrong verifier → invalid_grant (RFC 7636 §4.6).
    let resp = oauth2_token(&format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=cli&code_verifier=wrong-verifier"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "invalid_grant");

    // Correct verifier on a fresh code → 200.
    let (_, location) = oauth2_decide(&format!(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&email=u@example.com&decision=approve&code_challenge={challenge}&code_challenge_method=S256"
    ))
    .await;
    let code = oauth2_query_param(&location, "code").unwrap();
    let resp = oauth2_token(&format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=cli&code_verifier={verifier}"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn oauth2_pkce_plain_is_the_default_method() {
    // Omitting code_challenge_method defaults to "plain" (RFC 7636 §4.3).
    let verifier = "plain-verifier-value";
    let (_, location) = oauth2_decide(&format!(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&email=u@example.com&decision=approve&code_challenge={verifier}"
    ))
    .await;
    let code = oauth2_query_param(&location, "code").unwrap();
    let resp = oauth2_token(&format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=cli&code_verifier={verifier}"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn oauth2_decline_redirects_access_denied_with_state() {
    let (resp, location) = oauth2_decide(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&state=st4te&email=u@example.com&decision=decline",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        oauth2_query_param(&location, "error").as_deref(),
        Some("access_denied")
    );
    assert_eq!(
        oauth2_query_param(&location, "state").as_deref(),
        Some("st4te")
    );
    assert!(oauth2_query_param(&location, "code").is_none());
}

#[actix_web::test]
async fn oauth2_implicit_flow_fragment_redirect() {
    // §4.2.2: token in the fragment, never a refresh token; errors for the
    // implicit flow also go to the fragment (§4.2.2.1).
    let (resp, location) = oauth2_decide(
        "response_type=token&client_id=spa&redirect_uri=https://example.com/cb&state=imp&scope=profile&email=u@example.com&decision=approve",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let access = oauth2_fragment_param(&location, "access_token").expect("token in fragment");
    assert_eq!(
        oauth2_fragment_param(&location, "token_type").as_deref(),
        Some("Bearer")
    );
    assert_eq!(
        oauth2_fragment_param(&location, "expires_in").as_deref(),
        Some("3600")
    );
    assert_eq!(
        oauth2_fragment_param(&location, "state").as_deref(),
        Some("imp")
    );
    assert!(oauth2_query_param(&location, "access_token").is_none());
    assert!(oauth2_fragment_param(&location, "refresh_token").is_none());

    // The implicit access token works at userinfo like any other.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .insert_header(("Authorization", format!("Bearer {access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["client_id"], "spa");

    // Decline routes the error through the fragment for this flow.
    let (_, location) = oauth2_decide(
        "response_type=token&client_id=spa&redirect_uri=https://example.com/cb&state=imp&email=u@example.com&decision=decline",
    )
    .await;
    assert_eq!(
        oauth2_fragment_param(&location, "error").as_deref(),
        Some("access_denied")
    );
}

#[actix_web::test]
async fn oauth2_password_grant_issues_tokens() {
    let resp = oauth2_token(
        "grant_type=password&username=alice&password=hunter2&client_id=cli&scope=read",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["token_type"], "Bearer");
    assert!(v["refresh_token"].is_string());
    assert_eq!(v["scope"], "read");
    let access = v["access_token"].as_str().unwrap().to_string();

    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .insert_header(("Authorization", format!("Bearer {access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["sub"], "alice");

    // Missing username is invalid_request, not invalid_grant.
    let resp = oauth2_token("grant_type=password&password=x&client_id=cli").await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "invalid_request");
}

#[actix_web::test]
async fn oauth2_refresh_token_rotates() {
    let (_, location) = oauth2_decide(
        "response_type=code&client_id=cli&redirect_uri=https://example.com/cb&email=u@example.com&decision=approve",
    )
    .await;
    let code = oauth2_query_param(&location, "code").unwrap();
    let resp = oauth2_token(&format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https://example.com/cb&client_id=cli"
    ))
    .await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let refresh1 = v["refresh_token"].as_str().unwrap().to_string();
    let access1 = v["access_token"].as_str().unwrap().to_string();

    // Rotate: refresh1 → new pair.
    let resp = oauth2_token(&format!(
        "grant_type=refresh_token&refresh_token={refresh1}&client_id=cli"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let refresh2 = v["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(refresh1, refresh2);

    // refresh1 is spent; refresh2 still works; cross-client is rejected.
    let resp = oauth2_token(&format!(
        "grant_type=refresh_token&refresh_token={refresh1}&client_id=cli"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = oauth2_token(&format!(
        "grant_type=refresh_token&refresh_token={refresh2}&client_id=other"
    ))
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // A refresh token must not pass as an access token (type tag).
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .insert_header(("Authorization", format!("Bearer {refresh2}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // The original access token keeps working (access tokens aren't rotated).
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .insert_header(("Authorization", format!("Bearer {access1}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn oauth2_client_credentials_has_no_refresh_token() {
    // §4.4.3: SHOULD NOT issue a refresh token for client_credentials.
    let resp = oauth2_token(
        "grant_type=client_credentials&client_id=svc&client_secret=s3cr3t&scope=write",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(v.get("refresh_token").is_none());
    assert_eq!(v["scope"], "write");
    let access = v["access_token"].as_str().unwrap().to_string();

    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .insert_header(("Authorization", format!("Bearer {access}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(
        v["email"],
        crate::handlers::oauth2::CLIENT_CREDENTIALS_EMAIL
    );
}

#[actix_web::test]
async fn oauth2_basic_and_form_auth_and_invalid_client() {
    // Both RFC channels authenticate the same client.
    let resp =
        oauth2_token("grant_type=client_credentials&client_id=cli&client_secret=s3cr3t").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let app = test::init_service(create_app(cfg())).await;
    let credentials = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode("cli:s3cr3t")
    };
    let req = test::TestRequest::post()
        .uri("/oauth2/token")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .insert_header(("Authorization", format!("Basic {credentials}")))
        .set_payload("grant_type=client_credentials")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Configured registry: wrong secret → 401 invalid_client with the Basic
    // challenge (§5.2); right secret passes.
    let mut clients = std::collections::HashMap::new();
    clients.insert("cli".to_string(), "right".to_string());
    let app = test::init_service(create_app(
        ServerConfig::default().oauth2_clients(Some(clients)),
    ))
    .await;
    let req = test::TestRequest::post()
        .uri("/oauth2/token")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("grant_type=client_credentials&client_id=cli&client_secret=wrong")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(resp.headers().get("WWW-Authenticate").unwrap(), "Basic");
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "invalid_client");

    let req = test::TestRequest::post()
        .uri("/oauth2/token")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("grant_type=client_credentials&client_id=cli&client_secret=right")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn oauth2_token_errors_and_method_guard() {
    // Unknown grant.
    let resp = oauth2_token("grant_type=jwt-bearer&client_id=cli").await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "unsupported_grant_type");

    // Missing grant_type and missing code.
    let resp = oauth2_token("client_id=cli").await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "invalid_request");
    let resp = oauth2_token(
        "grant_type=authorization_code&client_id=cli&redirect_uri=https://example.com/cb",
    )
    .await;
    let v: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(v["error"], "invalid_request");

    // Token endpoint is POST-only: 405 + Allow: POST.
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/oauth2/token").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(resp.headers().get("Allow").unwrap(), "POST");

    // userinfo: bare challenge when no token, error attribute when invalid.
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(resp.headers().get("WWW-Authenticate").unwrap(), "Bearer");
    let req = test::TestRequest::get()
        .uri("/oauth2/userinfo")
        .insert_header(("Authorization", "Bearer forged.token"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get("WWW-Authenticate")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(challenge.contains("error=\"invalid_token\""));
}
