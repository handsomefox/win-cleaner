#!/usr/bin/env bash
# Installs CI tools from release binaries pinned by SHA-256.
#
# The hash is checked before the archive is read, so everything after that works on the exact
# bytes that were reviewed. Dependabot cannot bump these rows. To bump one, change its version,
# URL, and hash, taking the hash from the digest GitHub records for the release asset:
#
#   gh release view <tag> -R <owner>/<repo> --json assets \
#       --jq '.assets[] | select(.name == "<asset>") | .digest'
set -euo pipefail

# name version sha256 url
readonly tools=(
    "cargo-audit 0.22.2 7fb9497f8594b389e5fce5ef9b92db08432996895b2e0c5a0167a69ed445c428 https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.22.2/cargo-audit-x86_64-unknown-linux-musl-v0.22.2.tgz"
    "cargo-machete 0.9.2 48200087f54c55aabcd4db4af1e25742b49846c02a1b1bfa134711945b35b2e9 https://github.com/bnjbvr/cargo-machete/releases/download/v0.9.2/cargo-machete-v0.9.2-x86_64-unknown-linux-musl.tar.gz"
    "actionlint 1.7.12 8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8 https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz"
    "zizmor 1.30.1 e65324f4430c2717591937edcec90ccbefaf14c174f8ec9415e03ca875b46e1a https://github.com/zizmorcore/zizmor/releases/download/v1.30.1/zizmor-x86_64-unknown-linux-gnu.tar.gz"
)

if (($# == 0)); then
    echo "usage: install-ci-tool.sh <tool>..." >&2
    exit 2
fi

if [[ -n "${RUNNER_TEMP:-}" ]]; then
    bin_dir="$RUNNER_TEMP/ci-tools"
else
    bin_dir="$HOME/.local/bin"
fi
mkdir -p "$bin_dir"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

for name in "$@"; do
    row=
    for candidate in "${tools[@]}"; do
        if [[ "${candidate%% *}" == "$name" ]]; then
            row=$candidate
        fi
    done
    if [[ -z "$row" ]]; then
        echo "error: no pinned release for $name" >&2
        exit 2
    fi
    read -r _ version sha256 url <<< "$row"

    archive="$work/$name.tar.gz"
    curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 --retry 3 \
        --output "$archive" "$url"
    echo "$sha256  $archive" | sha256sum --check --strict --quiet

    mapfile -t entries < <(tar -tzf "$archive" | awk -F/ -v name="$name" '$NF == name')
    if ((${#entries[@]} != 1)); then
        echo "error: expected one $name in $url, found ${#entries[@]}" >&2
        exit 1
    fi
    tar -xzf "$archive" -C "$work" -- "${entries[0]}"
    install -m 755 "$work/${entries[0]}" "$bin_dir/$name"
    # sed reads to the end; head would close the pipe early and pipefail reports SIGPIPE.
    echo "installed $name $version: $("$bin_dir/$name" --version | sed -n 1p)"
done

if [[ -n "${GITHUB_PATH:-}" ]]; then
    echo "$bin_dir" >> "$GITHUB_PATH"
fi
