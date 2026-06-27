#!/usr/bin/env bash
# scripts/validate.sh — Post-deploy validation for AkurAI-IDP
# Exit 0 = healthy. Non-Framework Rust app (axum + rusqlite).
set -euo pipefail

DOMAIN="auth.olibuijr.com"
PORT=3500
RED='\033[0;31m'; GRN='\033[0;32m'; NC='\033[0m'
pass=0; fail=0
pass_() { printf "  ${GRN}PASS${NC} %s\n" "$*"; ((pass++)); }
fail_() { printf "  ${RED}FAIL${NC} %s\n" "$*"; ((fail++)); }

echo "=== Post-deploy validation: AkurAI-IDP ==="

# 1. Systemd
systemctl is-active --quiet akurai-idp.service 2>/dev/null && pass_ "systemd active" || fail_ "systemd not active"

# 2. Loopback health (IDP uses /health, not /api/health — needs standardization)
if curl -fsS --max-time 5 "http://127.0.0.1:${PORT}/health" > /dev/null 2>&1; then
  pass_ "loopback /health"
else
  fail_ "loopback /health unreachable"
fi

# 3. OIDC discovery
if curl -fsS --max-time 10 "https://${DOMAIN}/.well-known/openid-configuration" > /dev/null 2>&1; then
  pass_ "OIDC discovery endpoint"
else
  fail_ "OIDC discovery unreachable"
fi

# 4. Public HTTPS
STATUS=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 "https://${DOMAIN}/health" 2>/dev/null)
[ "$STATUS" = "200" ] && pass_ "public /health → 200" || fail_ "public /health → ${STATUS}"

# 5. Admin API (token from env)
IDP_ADMIN_TOKEN=$(sudo grep "^IDP_ADMIN_TOKEN=" /etc/akurai-idp/idp.env 2>/dev/null | cut -d= -f2-)
if [ -n "$IDP_ADMIN_TOKEN" ]; then
  ADMIN_STATUS=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 \
    -H "Authorization: Bearer $IDP_ADMIN_TOKEN" \
    "https://${DOMAIN}/admin/clients" 2>/dev/null)
  [ "$ADMIN_STATUS" = "200" ] && pass_ "admin API /admin/clients → 200" || fail_ "admin API → ${ADMIN_STATUS}"
else
  fail_ "IDP admin token not found"
fi

# 6. DB accessible
if [ -f "/data/akurai-idp/idp.sqlite" ]; then
  pass_ "SQLite DB file exists"
else
  fail_ "SQLite DB file missing"
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${GRN}Pass: $pass${NC}  ${RED}Fail: $fail${NC}"
[ "$fail" -eq 0 ] || exit 1
