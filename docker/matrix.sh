#!/usr/bin/env bash
# The CI matrix as JSON: one entry per Dockerfile.<target>, with the platforms and smoke test its
# `# hyprcross:` lines name. Arguments narrow it down to those targets.
#
#   [{"target":"aarch64-unknown-linux-gnu","platforms":"linux/amd64","smoke":"test"}, ...]

set -euo pipefail
cd "$(dirname "$0")"

wanted=" $* "
entries=()
for file in Dockerfile.*; do
    target="${file#Dockerfile.}"
    if [ $# -gt 0 ] && [[ "$wanted" != *" $target "* ]]; then
        continue
    fi
    platforms="$(sed -n 's/^# hyprcross:platforms //p' "$file")"
    smoke="$(sed -n 's/^# hyprcross:smoke //p' "$file")"
    entries+=("{\"target\":\"$target\",\"platforms\":\"${platforms:-linux/amd64}\",\"smoke\":\"${smoke:-build}\"}")
done

if [ ${#entries[@]} -eq 0 ]; then
    echo "matrix.sh: no Dockerfile for: $*" >&2
    exit 1
fi
(IFS=,; echo "[${entries[*]}]")
