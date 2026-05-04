#!/bin/sh
set -e

REPO="satoric-tech/bm25-cli"
BIN="bm25"
INSTALL_DIR="/usr/local/bin"

OS="$(uname -s)"
case "$OS" in
  Linux)  OS="unknown-linux-gnu" ;;
  Darwin) OS="apple-darwin" ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
esac

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64)          ARCH="x86_64" ;;
  arm64 | aarch64) ARCH="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

TARGET="${ARCH}-${OS}"

LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' \
  | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "Could not determine latest release. Check https://github.com/${REPO}/releases"
  exit 1
fi

ARCHIVE="${BIN}-${LATEST}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ARCHIVE}"

echo "Installing ${BIN} ${LATEST} (${TARGET})..."

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "$URL" -o "${TMP}/${ARCHIVE}"
tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"

if [ -w "$INSTALL_DIR" ]; then
  install -m 755 "${TMP}/${BIN}" "${INSTALL_DIR}/${BIN}"
else
  sudo install -m 755 "${TMP}/${BIN}" "${INSTALL_DIR}/${BIN}"
fi

echo "Installed to ${INSTALL_DIR}/${BIN}"
echo "Run: bm25 --help"
