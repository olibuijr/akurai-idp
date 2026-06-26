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
- Authenticated agent workspace at `/agent` for AkurAI-RustAgent interaction
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

The dedicated tenant agent host uses the tracked nginx vhost at
`ops/nginx/agent.olibuijr.com.conf` and proxies `https://agent.olibuijr.com/`
to the authenticated agent console route. The auth-domain redirect snippet at
`ops/nginx/auth-agent-redirect.conf` keeps the old `/agent` URL off
`auth.olibuijr.com`.

The first workspace slice is server-rendered and uses the stable RustAgent
`/query` gateway. It exposes channel/timeline, approval/question placeholders,
and durable context panes for memory, notes, passvault, cron, kanban, and
curator without assuming streaming or WebSocket support from the deployed
gateway.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `IDP_PORT` | `3500` | Listen port |
| `IDP_DB_PATH` | `./data/idp.sqlite` | SQLite database path |
| `IDP_BASE_URL` | `https://auth.olibuijr.com` | Public issuer URL |
| `IDP_ADMIN_TOKEN` | *(empty)* | Bearer token for admin API |
| `IDP_AGENT_PUBLIC_URL` | `https://agent.olibuijr.com` | Public URL for the tenant agent console |
| `IDP_AGENT_GATEWAY_URL` | `http://127.0.0.1:8644/query` | Rust Agent gateway query endpoint |
| `IDP_AGENT_ALLOWED_EMAILS` | `olibuijr@olibuijr.com` | Comma-separated console allowlist, or `*` |
| `IDP_AGENT_PROVIDER` | `openai-codex` | Agent provider sent to the gateway |
| `IDP_AGENT_MODEL` | `gpt-5.4-mini` | Agent model sent to the gateway |

## License

MIT
