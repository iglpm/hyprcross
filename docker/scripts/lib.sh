#!/usr/bin/env bash
# Helpers shared by the image build scripts. Sourced, never run.

set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

# Install packages and drop the apt lists in the same layer.
apt_install() {
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends "$@"
    rm -rf /var/lib/apt/lists/*
}

# Download `$1` to `$2`, retrying failures of any kind: mirrors reset connections now and
# then, and the URLs here are pinned, so a failure is never final on the first try.
fetch() {
    curl --fail --location --silent --show-error --connect-timeout 30 \
        --retry 5 --retry-delay 5 --retry-all-errors \
        --output "$2" "$1"
}

# Download the first of the URLs that has the file to `$1`. A 404 moves on to the next URL
# straight away; only failures that may pass are retried.
fetch_any() {
    local dest="$1"
    shift
    local url
    for url in "$@"; do
        if curl --fail --location --silent --show-error --connect-timeout 15 \
            --retry 3 --retry-delay 3 --retry-connrefused \
            --output "$dest" "$url"; then
            return 0
        fi
        echo "lib.sh: $url failed, trying the next mirror" >&2
    done
    echo "lib.sh: no mirror had $(basename "$dest")" >&2
    return 1
}
