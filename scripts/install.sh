#!/bin/bash
set -euo pipefail

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
  x86_64) ARCH="x64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

REPO="Varenik-vkusny/blackbox"
ASSET="blackbox-${OS}-${ARCH}.tar.gz"
URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"

echo "Downloading BlackBox..."
curl -fsSL "$URL" -o /tmp/blackbox.tar.gz

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"
tar -xzf /tmp/blackbox.tar.gz -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/blackbox"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> ~/.bashrc
  echo "Added $INSTALL_DIR to PATH. Run 'source ~/.bashrc' or restart shell."
fi

"$INSTALL_DIR/blackbox" setup --auto
