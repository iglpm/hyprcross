#!/usr/bin/env bash
# A FreeBSD sysroot at /opt/freebsd-sysroot, from the release's base.txz, and clang wrappers
# that compile and link against it (`freebsd.sh x86_64 amd64`).

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

arch="$1"
machine="$2"

# Binaries built against this run on it and every later release. Once a release is out of
# support it moves to the archive, which is where the second URL points.
FREEBSD_VERSION=14.4

apt_install clang lld llvm

release="${FREEBSD_VERSION}-RELEASE"
fetch_any /tmp/base.txz \
    "https://download.freebsd.org/releases/${machine}/${release}/base.txz" \
    "https://ftpmirror.your.org/pub/FreeBSD-Archive/old-releases/${machine}/${release}/base.txz" \
    "https://archive.freebsd.org/old-releases/${machine}/${release}/base.txz"

mkdir -p /opt/freebsd-sysroot
tar -xJf /tmp/base.txz -C /opt/freebsd-sysroot ./lib ./usr/lib ./usr/include
rm /tmp/base.txz

triple="${arch}-unknown-freebsd${FREEBSD_VERSION%%.*}"
for tool in clang clang++; do
    cat > "/usr/local/bin/${arch}-unknown-freebsd-${tool}" <<EOF
#!/bin/sh
exec ${tool} --target=${triple} --sysroot=/opt/freebsd-sysroot -fuse-ld=lld "\$@"
EOF
    chmod +x "/usr/local/bin/${arch}-unknown-freebsd-${tool}"
done

echo 'int main(void) { return 0; }' > /tmp/hello.c
"${arch}-unknown-freebsd-clang" -o /tmp/hello /tmp/hello.c
file /tmp/hello
rm -f /tmp/hello /tmp/hello.c
