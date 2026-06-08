#!/usr/bin/env bash
# Build gimoji-web for the browser and serve it on http://localhost:8000.
#
# Usage: ./scripts/serve-web.sh [PORT]
#
# Requirements (installed on first run via cargo / apt as needed):
#   * Rust toolchain with the wasm32-unknown-unknown target
#   * wasm-bindgen-cli
#   * (optional) wasm-opt — only used when present; the script still
#     produces a working bundle without it, just larger.
#   * python3 — for the static file server.
set -euo pipefail

PORT="${1:-8000}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WEB="$ROOT/crates/gimoji-web"
DIST="$WEB/web/dist"
# Cargo.lock is TOML; pull the version line that follows the wasm-bindgen
# package entry. Falls back to an empty string if not found.
WASM_BINDGEN_VERSION="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' \
    "$WEB/Cargo.lock" | head -1)"
if [[ -z "$WASM_BINDGEN_VERSION" ]]; then
    echo "error: could not determine wasm-bindgen version from $WEB/Cargo.lock" >&2
    exit 1
fi

step() { printf "\033[1;34m==>\033[0m %s\n" "$*"; }

if ! rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
    step "Installing wasm32-unknown-unknown target"
    rustup target add wasm32-unknown-unknown
fi

INSTALLED_BG_VER="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [[ "$INSTALLED_BG_VER" != "$WASM_BINDGEN_VERSION" ]]; then
    step "Installing wasm-bindgen-cli $WASM_BINDGEN_VERSION (have: ${INSTALLED_BG_VER:-none})"
    cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked
fi

step "Building gimoji-web with [profile.web]"
(cd "$WEB" && cargo build --profile web --locked)

step "Bundling with wasm-bindgen"
mkdir -p "$DIST"
wasm-bindgen --target web --no-typescript \
    --out-dir "$DIST" \
    "$WEB/target/wasm32-unknown-unknown/web/gimoji_web.wasm"

if command -v wasm-opt >/dev/null 2>&1; then
    step "Optimising with wasm-opt -Oz"
    wasm-opt -Oz \
        --enable-bulk-memory \
        --enable-nontrapping-float-to-int \
        -o "$DIST/gimoji_web_bg.wasm" \
        "$DIST/gimoji_web_bg.wasm"
else
    step "Skipping wasm-opt (binary not found; install binaryen for smaller bundles)"
fi

step "Copying HTML/CSS shell"
cp "$WEB/web/index.html" "$DIST/"
cp "$WEB/web/style.css" "$DIST/"

step "Bundle sizes"
ls -lh "$DIST"

step "Serving $DIST at http://localhost:$PORT (Ctrl+C to stop)"
exec python3 -m http.server -d "$DIST" "$PORT"
