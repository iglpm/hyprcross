#!/usr/bin/env bash
# Move the pinned upstream versions in scripts/ to the newest releases: wasi-sdk, Wasmtime and
# musl-cross-make. Prints what changed; CI turns the change into a pull request, whose image
# builds and smoke tests decide whether it merges.
#
# The Android NDK (an LTS line) and the FreeBSD sysroot (the oldest supported release) are
# chosen on purpose and moved by hand.

set -euo pipefail
cd "$(dirname "$0")"

api() {
    curl --fail --silent --show-error --location \
        ${GITHUB_TOKEN:+-H "Authorization: Bearer $GITHUB_TOKEN"} \
        -H "Accept: application/vnd.github+json" "https://api.github.com/repos/$1"
}

json_field() {
    python3 -c "import json,sys; print(json.load(sys.stdin)$1)"
}

# Replace `NAME=old` with `NAME=new` in a script; print the change if there is one.
bump() {
    local file="$1" name="$2" new="$3"
    local old
    old="$(sed -n "s/^${name}=//p" "$file")"
    if [ -n "$new" ] && [ "$old" != "$new" ]; then
        sed -i "s/^${name}=.*/${name}=${new}/" "$file"
        echo "${name}: ${old} -> ${new}"
    fi
}

wasi_sdk="$(api WebAssembly/wasi-sdk/releases/latest | json_field "['tag_name']")"
bump scripts/wasi.sh WASI_SDK_VERSION "${wasi_sdk#wasi-sdk-}"

wasmtime="$(api bytecodealliance/wasmtime/releases/latest | json_field "['tag_name']")"
bump scripts/wasi.sh WASMTIME_VERSION "$wasmtime"

mcm="$(api richfelker/musl-cross-make/commits/master | json_field "['sha']")"
bump scripts/musl.sh MCM_COMMIT "$mcm"
