#!/usr/bin/env bash
# Cross-builds the Windows release from Linux and packs it into dist/ the way a release does.
set -euo pipefail

readonly CARGO_XWIN_VERSION="0.23.1"
readonly bin="win-cleaner"
readonly package="cleaner-app"
readonly target="x86_64-pc-windows-msvc"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
metadata="$(cargo metadata --no-deps --format-version 1 --locked)"
target_dir="$(jq -er '.target_directory' <<< "$metadata")"
version="$(jq -er --arg package "$package" '.packages[] | select(.name == $package) | .version' <<< "$metadata")"

cargo_xwin_version="$(cargo xwin --version)"
if [[ ! "$cargo_xwin_version" =~ [[:space:]]${CARGO_XWIN_VERSION//./\.}$ ]]; then
    echo "cargo-xwin $CARGO_XWIN_VERSION is required; found: $cargo_xwin_version" >&2
    exit 1
fi

cargo xwin build --release --locked --target "$target" -p "$package"
"$root/scripts/package-release.sh" "$bin" "$version" \
    "windows-x86_64=$target_dir/$target/release/$bin.exe"
