#!/usr/bin/env bash
set -euo pipefail

readonly CARGO_XWIN_VERSION="0.23.0"
readonly product="win-cleaner"
readonly target="x86_64-pc-windows-msvc"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
artifact="$target_dir/$target/release/$product.exe"
dist="$root/dist"
zip_name="$product-windows-x86_64.zip"

cargo_xwin_version="$(cargo xwin --version)"
if [[ ! "$cargo_xwin_version" =~ [[:space:]]${CARGO_XWIN_VERSION//./\.}$ ]]; then
  echo "cargo-xwin $CARGO_XWIN_VERSION is required; found: $cargo_xwin_version" >&2
  exit 1
fi

cd "$root"
rm -rf "$dist"
mkdir -p "$dist"

CARGO_TARGET_DIR="$target_dir" cargo xwin build --workspace --release --locked --target "$target"
if [[ ! -f "$artifact" ]]; then
  echo "release executable not found: $artifact" >&2
  exit 1
fi
cp "$artifact" "$dist/$product.exe"

(
  cd "$dist"
  zip -9 -q "$zip_name" "$product.exe"
  sha256sum "$product.exe" "$zip_name" > SHA256SUMS

  mapfile -t expected_hash_assets < <(printf '%s\n' "$product.exe" "$zip_name" | sort)
  mapfile -t actual_hash_assets < <(awk '{print $2}' SHA256SUMS | sort)
  [[ "${actual_hash_assets[*]}" == "${expected_hash_assets[*]}" ]]
  awk 'length($1) != 64 || $1 !~ /^[0-9a-f]+$/ { exit 1 } END { if (NR != 2) exit 1 }' SHA256SUMS
  sha256sum --check --strict SHA256SUMS
  zip -T -q "$zip_name"
  mapfile -t entries < <(unzip -Z1 "$zip_name")
  [[ ${#entries[@]} -eq 1 && "${entries[0]}" == "$product.exe" ]]
)
