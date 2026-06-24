# akurai-idp

Lightweight Rust OIDC/OAuth2 identity provider for [AkurAI](https://github.com/olibuijr/AkurAI). Replaces the Node.js/Hono IDP (~96 MB RSS) with a single static binary (~3.3 MB RSS).

## Features

- Full OIDC/OAuth2 compliance: authorization code flow, refresh tokens, client credentials
- Ed25519 (EdDSA) JWT signing
- Argon2id password hashing
- TOTP-based MFA with backup codes
- PKCE support (S256 + plain)
- Token introspection (RFC 7662) and revocation (RFC 7009)
- Multi-tenant: users, groups, clients isolated by tenant
- Admin REST API with bearer token auth
- Browser UI: login, MFA, account settings (password, MFA, sessions)
- Rate limiting, CSRF protection, security headers
- SQLite storage (WAL mode)

## Build

```bash
# Dev
cargo build

# Release (static musl binary for Ubuntu/Debian)
CC_x86_64_unknown_linux_musl=musl-gcc cargo build --release --target x86_64-unknown-linux-musl
```

## Deploy

```bash
./deploy.sh
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `IDP_PORT` | `3500` | Listen port |
| `IDP_DB_PATH` | `./data/idp.sqlite` | SQLite database path |
| `IDP_BASE_URL` | `https://auth.olibuijr.com` | Public issuer URL |
| `IDP_ADMIN_TOKEN` | *(empty)* | Bearer token for admin API |

## License

MIT
