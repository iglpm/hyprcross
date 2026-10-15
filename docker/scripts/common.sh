#!/usr/bin/env bash
# What every image has: a native C toolchain for build scripts and proc-macros (they run on the
# build machine), and the tools common crates' build scripts reach for (cmake, perl for
# openssl-src, pkg-config, autotools).

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

apt_install \
    autoconf \
    automake \
    bzip2 \
    ca-certificates \
    cmake \
    curl \
    file \
    g++ \
    gcc \
    git \
    libc6-dev \
    libtool \
    m4 \
    make \
    ninja-build \
    patch \
    perl \
    pkg-config \
    python3 \
    unzip \
    xz-utils

# Containers run as the invoking user (podman --userns=keep-id, docker --user): make sure
# tools that want a writable HOME find one.
mkdir -p /home/hyprcross
chmod 1777 /home/hyprcross
