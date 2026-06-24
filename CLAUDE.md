# CLAUDE.md

## What this is

Rust OIDC/OAuth2 identity provider for AkurAI. Full port of the Node.js/Hono IDP. Serves login/MFA pages, issues JWTs, manages users/groups/tenants/clients via admin API. Runs on AWS EC2 (Ubuntu 22.04).

## Build & Deploy

- `CC_x86_64_unknown_linux_musl=musl-gcc cargo build --release --target x86_64-unknown-linux-musl`
- `./deploy.sh` — builds musl binary, uploads to VM, restarts systemd service, healthchecks
- VM: `ssh akurai-mail` (3.94.46.219, Ubuntu 22.04, systemd `akurai-idp.service`, user `idp`)
- Data: `/data/akurai-idp/idp.sqlite`, env: `/etc/akurai-idp/idp.env`

## Architecture

```
nginx (TLS, auth.olibuijr.com) → akurai-idp (:3500) → SQLite
```

- `src/main.rs` — router wiring, admin auth middleware, signing key bootstrap
- `src/config.rs` — env var config (lazy static)
- `src/db/` — SQLite via rusqlite, schema creation, `with_db()` accessor
- `src/lib/` — crypto (SHA256, argon2, Ed25519 JWT, TOTP, PKCE, audit logging, HTML templates)
- `src/middleware/` — secure headers, rate limiting, CSRF, session/bearer/admin auth
- `src/routes/` — OIDC endpoints (authorize, token, introspect, revoke, userinfo, well-known)
- `src/routes/auth_pages.rs` — login, MFA, logout HTML pages
- `src/routes/account.rs` — password change, MFA setup, session management
- `src/routes/admin/` — REST API for users, groups, tenants, clients, audit log

## Environment Variables

| Variable | Default | Required |
|----------|---------|----------|
| `IDP_PORT` | `3500` | No |
| `IDP_DB_PATH` | `./data/idp.sqlite` | No |
| `IDP_BASE_URL` | `https://auth.olibuijr.com` | Yes |
| `IDP_ADMIN_TOKEN` | *(empty)* | Yes (for admin API) |

## Constraints

- Binary MUST cross-compile with `x86_64-unknown-linux-musl` (needs `CC_x86_64_unknown_linux_musl=musl-gcc`)
- Same SQLite schema as the Node.js version — data is shared, migration-free
- Ed25519 (EdDSA) for all JWT signing — no RSA
- Argon2id for password hashing (64MB memory cost)
- TOTP with ±1 step window for MFA
- PKCE support (S256 + plain)
- RSS must stay under 10 MB
- Release profile: opt-level=z, LTO, strip, panic=abort

## OIDC Endpoints

- `GET /.well-known/openid-configuration` — discovery document
- `GET /jwks` — JSON Web Key Set
- `GET /authorize` — authorization code flow
- `POST /token` — token exchange (auth_code, refresh_token, client_credentials)
- `GET /userinfo` — bearer-protected user claims
- `POST /introspect` — RFC 7662
- `POST /revoke` — RFC 7009

## Admin API (Bearer token)

- `/admin/users` — CRUD + lock/unlock/reset-password
- `/admin/groups` — CRUD + member management
- `/admin/tenants` — CRUD
- `/admin/clients` — CRUD + secret rotation
- `/admin/audit` — query audit log

## Testing

- `cargo check` — compilation
- `cargo build --release --target x86_64-unknown-linux-musl` — release cross-compile
- Deploy healthcheck: `curl http://127.0.0.1:3500/health` → `{"ok":true}`
- OIDC discovery: `curl https://auth.olibuijr.com/.well-known/openid-configuration`

## Related

- **Mail API:** [olibuijr/akurai-mail-api](https://github.com/olibuijr/akurai-mail-api) — same VM, port 3000
- **Mail Frontend:** [olibuijr/AkurAIMail](https://github.com/olibuijr/AkurAIMail) — SvelteKit SPA
- **Original Node IDP:** [olibuijr/AkurAIIDP](https://github.com/olibuijr/AkurAIIDP) — replaced by this
