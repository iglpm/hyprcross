#!/usr/bin/env bash
# A musl cross toolchain built from source with musl-cross-make, installed to /usr/local/musl:
#   musl.sh <gcc-target> [extra gcc configure flags...]
#   musl.sh armv7-linux-musleabihf --with-arch=armv7-a --with-float=hard
#
# Distributions don't package musl cross compilers, and prebuilt ones come from hosts that rate
# limit CI. Building takes a while, so Dockerfiles run this in a stage of its own: the layer is
# reused until this script or the base image changes.

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

target="$1"
shift

# The commit decides which gcc, binutils and musl versions (and patches) are available.
MCM_COMMIT=227df8b99103f9c59f6570babf892978e293082f
GCC_VER=14.3.0
BINUTILS_VER=2.44
# The musl Rust's own *-musl targets bundle: C code compiled here links against Rust's libc.a.
MUSL_VER=1.2.5
LINUX_VER=6.15.7

apt_install rsync

work="$(mktemp -d)"
fetch "https://github.com/richfelker/musl-cross-make/archive/${MCM_COMMIT}.tar.gz" "$work/mcm.tar.gz"
tar -xzf "$work/mcm.tar.gz" -C "$work" --strip-components=1

cat > "$work/config.mak" <<EOF
TARGET = ${target}
OUTPUT = /usr/local/musl
GCC_VER = ${GCC_VER}
BINUTILS_VER = ${BINUTILS_VER}
MUSL_VER = ${MUSL_VER}
LINUX_VER = ${LINUX_VER}
DL_CMD = curl --fail --location --silent --show-error --retry 5 --retry-delay 5 --retry-all-errors -C - -o
COMMON_CONFIG += CFLAGS="-g0 -O2" CXXFLAGS="-g0 -O2" LDFLAGS="-s"
GCC_CONFIG += --enable-languages=c,c++ --disable-libsanitizer --disable-multilib $*
EOF

make -C "$work" -j"$(nproc)" install
rm -rf "$work"

# musl installs its dynamic loader as an absolute link to /lib/libc.so, which under QEMU's
# sysroot prefix points out of the sysroot. Dynamically linked programs (Rust's
# riscv64gc-unknown-linux-musl is one by default) need it to resolve inside it.
for loader in "/usr/local/musl/${target}"/lib/ld-musl-*.so.1; do
    if [ -L "$loader" ]; then
        ln -sf libc.so "$loader"
    fi
done

# The toolchain's own sanity check: compile, link statically, and look at the result.
cat > /tmp/hello.c <<'EOF'
int main(void) { return 0; }
EOF
"/usr/local/musl/bin/${target}-gcc" -static -o /tmp/hello /tmp/hello.c
file /tmp/hello
rm -f /tmp/hello /tmp/hello.c
