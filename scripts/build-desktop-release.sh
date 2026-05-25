#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required but was not found in PATH" >&2
  exit 1
fi

if ! cargo tauri --help >/dev/null 2>&1; then
  echo "cargo tauri is required; install the Tauri CLI before building release bundles" >&2
  exit 1
fi

"$repo_root/scripts/generate-desktop-icons.sh"

cd "$repo_root/apps/desktop/src-tauri"
cargo tauri build "$@"
