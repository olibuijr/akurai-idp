#!/usr/bin/env bash
# Deploy rules of conduct:
# - Read AGENTS.md and canonical Notes/docs before changing or deploying.
# - Never print, commit, or copy secrets; use passvault/env files only.
# - Preserve env files, service users, user data, and databases; snapshot state before risky swaps.
# - Verify ports, DNS, TLS, and systemd unit names before changing routes or services.
# - Use managed services only (systemd/pm2); no nohup, background shells, or ad hoc daemons.
# - Run gates and health checks; if deploy fails, stop and roll back rather than improvising.
# - Keep deploy behavior unchanged unless the task explicitly asks for deploy logic changes.
set -euo pipefail

SSH_HOST="${AKURAI_MAIL_SSH:-akurai-mail}"
DEPLOY_DIR="/opt/akurai-idp"
SERVICE="akurai-idp"
TARGET="x86_64-unknown-linux-musl"

log()  { echo "[deploy] $*"; }
step() { echo; echo "── $* ──"; }

step "1/4  Build"
CC_x86_64_unknown_linux_musl=musl-gcc cargo build --release --target "$TARGET" 2>&1
BINARY="target/$TARGET/release/akurai-idp"
log "Binary: $(du -sh "$BINARY" | cut -f1)"

step "2/4  Upload"
ssh "$SSH_HOST" "sudo mkdir -p $DEPLOY_DIR"
scp -q "$BINARY" "$SSH_HOST:/tmp/akurai-idp-bin"
ssh "$SSH_HOST" "sudo install -m 755 /tmp/akurai-idp-bin $DEPLOY_DIR/akurai-idp && rm /tmp/akurai-idp-bin"

step "3/4  Install service"
ssh "$SSH_HOST" "sudo bash -s" <<'INSTALL'
set -euo pipefail

cat > /etc/systemd/system/akurai-idp.service <<EOF
[Unit]
Description=AkurAI IDP (Rust OIDC Provider)
After=network.target

[Service]
Type=simple
ExecStart=/opt/akurai-idp/akurai-idp
WorkingDirectory=/opt/akurai-idp
EnvironmentFile=/etc/akurai-idp/idp.env
Restart=always
RestartSec=3
User=idp
Group=idp

[Install]
WantedBy=multi-user.target
EOF

mkdir -p /data/akurai-idp
chown idp:idp /data/akurai-idp

systemctl daemon-reload
systemctl restart akurai-idp
echo "  [done] service restarted"
INSTALL

step "4/4  Healthcheck"
sleep 2
if ssh "$SSH_HOST" "curl -fsS http://127.0.0.1:3500/health" | grep -q '"ok":true'; then
  log "Healthcheck passed"
else
  echo "ERROR: healthcheck failed"
  echo "Check: ssh $SSH_HOST 'sudo journalctl -u $SERVICE -n 50'"
  exit 1
fi

RSS=$(ssh "$SSH_HOST" "ps -C akurai-idp -o rss= 2>/dev/null | head -1 | tr -d ' '" 2>/dev/null || echo "?")
echo
echo "════════════════════════════════════════════════════"
echo "  AkurAI IDP deployed (Rust)"
echo "════════════════════════════════════════════════════"
echo "  Binary  : $(du -sh "$BINARY" | cut -f1)"
echo "  RSS     : ${RSS} KB"
echo "  Host    : $SSH_HOST"
echo "  Public  : https://auth.olibuijr.com"
echo "════════════════════════════════════════════════════"
