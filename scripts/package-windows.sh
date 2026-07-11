#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="x86_64-pc-windows-msvc"
artifact="${CARGO_TARGET_DIR:-$root/target}/${target}/release/win-cleaner.exe"
dist="$root/dist"

cd "$root"
cargo xwin build --workspace --release --target "$target"
mkdir -p "$dist"
cp "$artifact" "$dist/win-cleaner.exe"
(
  cd "$dist"
  sha256sum win-cleaner.exe > SHA256SUMS
  zip -9 -q win-cleaner-windows-x86_64.zip win-cleaner.exe SHA256SUMS
)
