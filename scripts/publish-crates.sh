#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: scripts/publish-crates.sh <dry-run|publish> [--from <crate>]

publish order:
  mf2_i18n_core
  mf2_i18n_std
  mf2_i18n_build
  mf2_i18n_embedded
  mf2_i18n_runtime
  mf2_i18n_native
  mf2_i18n_server
  mf2_i18n_leptos
  mf2_i18n_wasm
  mf2_i18n
  mf2_i18n_cli

notes:
  - dry-run lists each crate's packaged contents locally
  - publish uploads crates to crates.io in dependency order
  - --from resumes from the named crate
EOF
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

mode="$1"
shift

from_crate=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --from)
      shift
      if [[ $# -eq 0 ]]; then
        echo "--from requires a crate name" >&2
        exit 1
      fi
      from_crate="$1"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

case "$mode" in
  dry-run|publish)
    ;;
  *)
    echo "invalid mode: $mode" >&2
    usage
    exit 1
    ;;
esac

publish_order=(
  mf2_i18n_core
  mf2_i18n_std
  mf2_i18n_build
  mf2_i18n_embedded
  mf2_i18n_runtime
  mf2_i18n_native
  mf2_i18n_server
  mf2_i18n_leptos
  mf2_i18n_wasm
  mf2_i18n
  mf2_i18n_cli
)

if [[ -n "$from_crate" ]]; then
  found_from=0
  for crate in "${publish_order[@]}"; do
    if [[ "$crate" == "$from_crate" ]]; then
      found_from=1
      break
    fi
  done

  if [[ "$found_from" -ne 1 ]]; then
    echo "unknown crate in --from: $from_crate" >&2
    exit 1
  fi
fi

run_dry_run() {
  local crate="$1"
  echo "==> listing packaged contents for $crate"
  cargo package -p "$crate" --list --locked --allow-dirty
}

run_publish() {
  local crate="$1"
  local max_attempts=12
  local attempt=1
  local log_file
  log_file="$(mktemp)"

  while true; do
    echo "==> publishing $crate (attempt $attempt/$max_attempts)"
    if cargo publish -p "$crate" --locked 2>&1 | tee "$log_file"; then
      rm -f "$log_file"
      return 0
    fi

    if grep -qi 'already uploaded' "$log_file"; then
      echo "==> $crate already exists on crates.io; treating as success"
      rm -f "$log_file"
      return 0
    fi

    if [[ "$attempt" -ge "$max_attempts" ]]; then
      echo "failed to publish $crate after $max_attempts attempts" >&2
      rm -f "$log_file"
      return 1
    fi

    attempt=$((attempt + 1))
    echo "==> waiting for crates.io index propagation before retry"
    sleep 20
  done
}

should_run=1
for crate in "${publish_order[@]}"; do
  if [[ -n "$from_crate" && "$should_run" -eq 0 ]]; then
    if [[ "$crate" == "$from_crate" ]]; then
      should_run=1
    else
      continue
    fi
  fi

  if [[ "$mode" == "dry-run" ]]; then
    run_dry_run "$crate"
  else
    run_publish "$crate"
  fi
done
