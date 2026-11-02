# OAuth 2.0 mock (`/oauth2`)

A working authorization server for testing OAuth clients, with a consent page, token endpoint, Bearer-protected userinfo, and discovery metadata.

```bash
# 1) Send the resource owner to the authorization endpoint (consent page)
open "https://httpcan.org/oauth2/authorize?response_type=code&client_id=webapp&redirect_uri=https%3A%2F%2Fexample.com%2Fcb&state=xyz&scope=read"

# 2) Exchange the one-time code (Basic or form-body client auth; PKCE supported)
curl -X POST https://httpcan.org/oauth2/token \
  -u webapp:s3cr3t \
  -d "grant_type=authorization_code&code=<code>&redirect_uri=https://example.com/cb"

# 3) Call the Bearer-protected resource
curl https://httpcan.org/oauth2/userinfo -H "Authorization: Bearer <token>"
```

## Supported flows

- The four RFC 6749 grants:
  - `authorization_code`, with PKCE (`S256` or `plain`)
  - implicit (`response_type=token`, legacy): the access token comes back in the redirect fragment
  - `password`
  - `client_credentials`
- `refresh_token`, with rotation: each refresh token works only once.

Codes are one-time, tokens are HMAC-signed per process, and the email entered on the consent page is the only identity. `GET /oauth2` is a self-describing index, and `GET /.well-known/oauth-authorization-server` serves RFC 8414 metadata for clients that use discovery.

## Client validation

By default any non-empty client secret is accepted (mock mode). To validate secrets, register clients with `--oauth2-clients "webapp:s3cr3t,cli:an0ther"` (or `HTTPCAN_OAUTH2_CLIENTS`). A mismatch then returns `invalid_client`. See [configuration](configuration.md).
