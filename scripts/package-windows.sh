#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="x86_64-pc-windows-msvc"
dist="$root/dist"

cd "$root"
target_dir="$(cargo metadata --no-deps --format-version 1 | jq -r '.target_directory')"
artifact="$target_dir/$target/release/win-cleaner.exe"

cargo xwin build --workspace --release --target "$target"
mkdir -p "$dist"
cp "$artifact" "$dist/win-cleaner.exe"
(
  cd "$dist"
  sha256sum win-cleaner.exe > SHA256SUMS
  zip -9 -q win-cleaner-windows-x86_64.zip win-cleaner.exe SHA256SUMS
)
