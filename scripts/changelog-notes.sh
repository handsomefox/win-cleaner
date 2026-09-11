#!/usr/bin/env bash
# Prints the CHANGELOG.md section for one version, which becomes that release's notes.
# Fails when the section is missing or empty, so a release cannot go out without notes.
set -euo pipefail

version=${1:?usage: changelog-notes.sh <version>}
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# The section runs from its "## <version>" heading to the next "## " heading. sed drops the
# blank lines before the first line of text, and $(...) drops the ones after the last.
notes=$(awk -v heading="## $version" '
    $0 == heading { found = 1; next }
    found && /^## / { exit }
    found { print }
' "$root/CHANGELOG.md" | sed '/./,$!d')

if [[ -z "${notes//[[:space:]]/}" ]]; then
    echo "error: CHANGELOG.md has no notes under '## $version'" >&2
    exit 1
fi
printf '%s\n' "$notes"
