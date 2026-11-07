#!/bin/bash
# One-shot setup for the Earthdrop multiplayer relay on a Linux box.
# Run as root (or with sudo) on the target machine.
# Usage: curl -fsSL <repo-raw-url>/tools/setup_server.sh | sudo bash -s -- <repo-url> [port]
set -euo pipefail

REPO="${1:-https://github.com/curnpoo/EveryWorld.git}"
PORT="${2:-7777}"
INSTALL_DIR="/opt/earthdrop"
BIN="/usr/local/bin/earthdrop-server"
SERVICE="/etc/systemd/system/earthdrop-server.service"

echo "==> Installing build deps"
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq git curl build-essential pkg-config libssl-dev > /dev/null 2>&1 || true

if ! command -v cargo >/dev/null 2>&1; then
  echo "==> Installing Rust (rustup)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  export PATH="$HOME/.cargo/bin:$PATH"
fi

echo "==> Cloning/updating $REPO"
if [ -d "$INSTALL_DIR/.git" ]; then
  git -C "$INSTALL_DIR" pull --ff-only
else
  rm -rf "$INSTALL_DIR"
  git clone --depth 1 "$REPO" "$INSTALL_DIR"
fi
cd "$INSTALL_DIR"
export PATH="$HOME/.cargo/bin:$PATH"

echo "==> Building server (release)"
cargo build --release -p ed_server
install -m 0755 target/release/earthdrop-server "$BIN"

echo "==> Installing systemd service"
cat > "$SERVICE" << EOF
[Unit]
Description=Earthdrop multiplayer relay
After=network.target

[Service]
ExecStart=$BIN $PORT
Restart=always
RestartSec=3
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now earthdrop-server
systemctl is-active --quiet earthdrop-server && echo "service: active" || (echo "service failed:"; systemctl status earthdrop-server --no-pager; exit 1)

echo "==> Firewall (ufw if present)"
if command -v ufw >/dev/null 2>&1; then
  ufw allow "$PORT"/udp || true
  ufw --force enable || true
fi
# firewalld (RHEL/CentOS/Fedora)
if command -v firewall-cmd >/dev/null 2>&1; then
  firewall-cmd --permanent --add-port="$PORT"/udp || true
  firewall-cmd --reload || true
fi

# Also try to open the port via the kernel directly if no manager
if command -v iptables >/dev/null 2>&1; then
  iptables -I INPUT -p udp --dport "$PORT" -j ACCEPT 2>/dev/null || true
fi

echo
echo "==> DONE. Server is listening on UDP $PORT."
echo "    Local test:  ss -u -l -n | grep $PORT"
ss -u -l -n | grep "$PORT" || netstat -u -l -n 2>/dev/null | grep "$PORT" || echo "    (could not verify listener, check: systemctl status earthdrop-server)"
echo
echo "    Public IP:   curl -s https://ifconfig.me ; echo"
echo "    Friends connect with:"
echo "      earthdrop --server <PUBLIC_IP>:$PORT --name <name>"
echo
echo "    NOTE: Tailscale Funnel / ngrok / Cloudflare Tunnel do NOT support UDP."
echo "    This server needs a real public UDP path:"
echo "      - VPS with public IP (works as-is), OR"
echo "      - Home box behind a router: forward UDP $PORT in the router to this machine,"
echo "        then friends use your router's public IP."
echo "    Verify from another machine:  nc -u -z <PUBLIC_IP> $PORT  (should not hang/timeout)"
