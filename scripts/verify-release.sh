#!/usr/bin/env bash
# Checks a dist/ folder holds exactly the release archives for the given platforms and a
# SHA256SUMS that matches them, and that each archive holds exactly the expected files.
set -euo pipefail

if (($# < 4)); then
    echo "usage: verify-release.sh <dist> <bin> <version> <platform>..." >&2
    exit 2
fi
dist=$1
bin=$2
version=$3
shift 3

# Prints a diff and fails when the two newline-separated lists differ.
same_list() {
    local what=$1 expected=$2 actual=$3
    if [[ "$expected" != "$actual" ]]; then
        echo "error: unexpected $what" >&2
        diff -u <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") >&2 || true
        exit 1
    fi
}

archives=()
for platform in "$@"; do
    stem="$bin-$version-$platform"
    case "$platform" in
        windows-*)
            executable="$bin.exe"
            archive="$stem.zip"
            unzip -tq "$dist/$archive" > /dev/null
            entries=$(unzip -Z1 "$dist/$archive" | sort)
            ;;
        linux-*)
            executable="$bin"
            archive="$stem.tar.gz"
            entries=$(tar -tzf "$dist/$archive" | sort)
            read -r mode _ < <(tar -tvzf "$dist/$archive" -- "$stem/$executable")
            if [[ "$mode" != -rwxr-xr-x ]]; then
                echo "error: $stem/$executable has mode $mode, not -rwxr-xr-x" >&2
                exit 1
            fi
            ;;
        *)
            echo "error: unknown platform $platform" >&2
            exit 2
            ;;
    esac
    same_list "entries in $archive" \
        "$(printf '%s\n' "$stem/" "$stem/$executable" "$stem/LICENSE" "$stem/README.md" | sort)" \
        "$entries"
    archives+=("$archive")
done

same_list "files in $dist" \
    "$(printf '%s\n' SHA256SUMS "${archives[@]}" | sort)" \
    "$(find "$dist" -mindepth 1 -maxdepth 1 -printf '%f\n' | sort)"

hashed=()
while IFS= read -r line; do
    if [[ ! "$line" =~ ^[0-9a-f]{64}\ \ (.+)$ ]]; then
        echo "error: malformed SHA256SUMS line: $line" >&2
        exit 1
    fi
    hashed+=("${BASH_REMATCH[1]}")
done < "$dist/SHA256SUMS"
same_list "files in SHA256SUMS" \
    "$(printf '%s\n' "${archives[@]}" | sort)" \
    "$(printf '%s\n' "${hashed[@]}" | sort)"
(cd "$dist" && sha256sum --check --strict --quiet SHA256SUMS)

echo "verified ${archives[*]} and SHA256SUMS"
