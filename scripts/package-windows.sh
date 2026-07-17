#!/usr/bin/env bash
set -euo pipefail

readonly CARGO_XWIN_VERSION="0.23.0"
readonly product="win-cleaner"
readonly executable_name="Windows Cleaner.exe"
readonly target="x86_64-pc-windows-msvc"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dist="$root/dist"
zip_name="$product-windows-x86_64.zip"

cd "$root"
target_dir="$(cargo metadata --no-deps --format-version 1 --locked | jq -er '.target_directory')"
artifact="$target_dir/$target/release/$product.exe"

cargo_xwin_version="$(cargo xwin --version)"
if [[ ! "$cargo_xwin_version" =~ [[:space:]]${CARGO_XWIN_VERSION//./\.}$ ]]; then
  echo "cargo-xwin $CARGO_XWIN_VERSION is required; found: $cargo_xwin_version" >&2
  exit 1
fi

rm -rf "$dist"
mkdir -p "$dist"

cargo xwin build --workspace --release --locked --target "$target"
if [[ ! -f "$artifact" ]]; then
  echo "release executable not found: $artifact" >&2
  exit 1
fi
cp "$artifact" "$dist/$executable_name"

(
  cd "$dist"
  zip -9 -q "$zip_name" "$executable_name"
  sha256sum "$executable_name" "$zip_name" > SHA256SUMS

  mapfile -t expected_hash_assets < <(printf '%s\n' "$executable_name" "$zip_name" | sort)
  mapfile -t actual_hash_assets < <(cut -c67- SHA256SUMS | sort)
  [[ "${actual_hash_assets[*]}" == "${expected_hash_assets[*]}" ]]
  awk 'length($1) != 64 || $1 !~ /^[0-9a-f]+$/ { exit 1 } END { if (NR != 2) exit 1 }' SHA256SUMS
  sha256sum --check --strict SHA256SUMS
  zip -T -q "$zip_name"
  mapfile -t entries < <(unzip -Z1 "$zip_name")
  [[ ${#entries[@]} -eq 1 && "${entries[0]}" == "$executable_name" ]]
)
