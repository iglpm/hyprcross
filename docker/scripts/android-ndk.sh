#!/usr/bin/env bash
# The Android NDK's LLVM toolchain at /opt/android-ndk (`android-ndk.sh aarch64-linux-android`),
# with the sysroot libraries of other architectures removed.

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

keep="$1"

# The current long-term-support NDK.
NDK_VERSION=r27c

fetch "https://dl.google.com/android/repository/android-ndk-${NDK_VERSION}-linux.zip" /tmp/ndk.zip
unzip -q /tmp/ndk.zip -d /opt
rm /tmp/ndk.zip
mv "/opt/android-ndk-${NDK_VERSION}" /opt/android-ndk

# Building and linking needs the LLVM toolchain, the sysroot and the CMake/ndk-build glue.
cd /opt/android-ndk
rm -rf simpleperf shader-tools python-packages prebuilt sources/third_party
lib=toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib
for dir in "$lib"/*-linux-android*; do
    if [ "$(basename "$dir")" != "$keep" ]; then
        rm -rf "$dir"
    fi
done
test -d "$lib/$keep"
