#!/usr/bin/env bash

set -euo pipefail

VERSION="0.1.0" # x-release-please-version
REPOSITORY="ponko2/herdr-equalize-panes"
BINARY="equalize-panes"

case "$(uname -s):$(uname -m)" in
Darwin:arm64)
  TARGET="aarch64-apple-darwin"
  ;;
Linux:x86_64 | Linux:amd64)
  TARGET="x86_64-unknown-linux-gnu"
  ;;
*)
  TARGET=""
  ;;
esac

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DESTINATION_DIR="$SCRIPT_DIR/bin"

if [[ -z "$TARGET" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Unsupported platform: $(uname -s) $(uname -m). Install Rust and Cargo to build from source." >&2
    exit 1
  fi

  echo "No pre-built binary for $(uname -s) $(uname -m); building from source." >&2
  (
    cd "$SCRIPT_DIR"
    cargo build --locked --release
  )
  mkdir -p "$DESTINATION_DIR"
  install -m 755 "$SCRIPT_DIR/target/release/$BINARY" "$DESTINATION_DIR/$BINARY"
  exit 0
fi

ASSET="$BINARY-$VERSION-$TARGET.tar.gz"
RELEASE_URL="https://github.com/$REPOSITORY/releases/download/v$VERSION"
WORK_DIR=$(mktemp -d)

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT HUP INT TERM

curl -fsSL -o "$WORK_DIR/$ASSET" "$RELEASE_URL/$ASSET"
curl -fsSL -o "$WORK_DIR/$ASSET.sha256" "$RELEASE_URL/$ASSET.sha256"

(
  cd "$WORK_DIR"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum --check "$ASSET.sha256"
  else
    shasum -a 256 -c "$ASSET.sha256"
  fi
)

mkdir -p "$DESTINATION_DIR"
tar --extract --gzip --file "$WORK_DIR/$ASSET" --directory "$WORK_DIR" "$BINARY"
install -m 755 "$WORK_DIR/$BINARY" "$DESTINATION_DIR/$BINARY"
