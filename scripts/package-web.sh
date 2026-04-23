#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: scripts/package-web.sh [web|bundler|all]

builds local JavaScript packages:
  web      pkg/mf2_i18n_wasm-web
  bundler  pkg/mf2_i18n_wasm-bundler
  all      both packages
EOF
}

mode="${1:-all}"
case "$mode" in
  -h|--help)
    usage
    exit 0
    ;;
  web|bundler|all)
    ;;
  *)
    echo "invalid mode: $mode" >&2
    usage
    exit 1
    ;;
esac

if [[ $# -gt 1 ]]; then
  echo "unexpected argument: $2" >&2
  usage
  exit 1
fi

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "missing required tool: wasm-pack" >&2
  echo "install with: cargo install wasm-pack --locked" >&2
  exit 127
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

build_target() {
  local target="$1"
  (
    cd "$repo_root/crates/mf2_i18n_wasm"
    wasm-pack build . \
      --target "$target" \
      --out-dir "../../pkg/mf2_i18n_wasm-$target" \
      --out-name mf2_i18n_wasm \
      --release \
      --no-opt \
      -- --locked
  )
}

case "$mode" in
  web)
    build_target web
    ;;
  bundler)
    build_target bundler
    ;;
  all)
    build_target web
    build_target bundler
    ;;
esac
