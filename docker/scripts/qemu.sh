#!/usr/bin/env bash
# QEMU user-mode emulation for one architecture (`qemu.sh aarch64`): what `hyprcross test` and
# `hyprcross run` execute target programs under.

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

arch="$1"

apt_install qemu-user-static

# The package ships every architecture QEMU knows; this image runs one.
find /usr/bin -maxdepth 1 -name 'qemu-*' \
    ! -name "qemu-${arch}-static" ! -name "qemu-${arch}" -delete
test -x "/usr/bin/qemu-${arch}-static"
