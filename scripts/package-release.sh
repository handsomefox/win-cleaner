#!/usr/bin/env bash
# Packs release binaries into versioned archives under dist/ and writes SHA256SUMS.
#
# A windows-* platform becomes a .zip and a linux-* platform a .tar.gz. Each archive holds one
# folder, <bin>-<version>-<platform>/, with the binary, README.md, and LICENSE in it.
set -euo pipefail

if (($# < 3)); then
    echo "usage: package-release.sh <bin> <version> <platform>=<binary>..." >&2
    exit 2
fi
bin=$1
version=$2
shift 2

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dist="$root/dist"
rm -rf "$dist"
mkdir -p "$dist"

platforms=()
archives=()
for spec in "$@"; do
    platform=${spec%%=*}
    binary=${spec#*=}
    if [[ "$platform" == "$spec" || ! -f "$binary" ]]; then
        echo "error: expected <platform>=<existing binary>, got $spec" >&2
        exit 2
    fi
    case "$platform" in
        windows-*) executable="$bin.exe" archive_suffix=zip ;;
        linux-*) executable="$bin" archive_suffix=tar.gz ;;
        *)
            echo "error: unknown platform $platform" >&2
            exit 2
            ;;
    esac

    stem="$bin-$version-$platform"
    mkdir "$dist/$stem"
    install -m 755 "$binary" "$dist/$stem/$executable"
    install -m 644 "$root/README.md" "$root/LICENSE" "$dist/$stem/"
    archive="$stem.$archive_suffix"
    if [[ "$archive_suffix" == zip ]]; then
        (cd "$dist" && zip -9 -q -X -r "$archive" "$stem")
    else
        tar -czf "$dist/$archive" --sort=name --owner=0 --group=0 --numeric-owner -C "$dist" "$stem"
    fi
    rm -rf "${dist:?}/$stem"

    platforms+=("$platform")
    archives+=("$archive")
done

(cd "$dist" && sha256sum -- "${archives[@]}" > SHA256SUMS)
"$root/scripts/verify-release.sh" "$dist" "$bin" "$version" "${platforms[@]}"
