#!/usr/bin/env bash
# wasi-sdk (clang and a WASI sysroot, for C dependencies) at /opt/wasi-sdk, and Wasmtime to run
# tests and programs.

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

WASI_SDK_VERSION=34
WASMTIME_VERSION=v49.0.3

fetch "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-${WASI_SDK_VERSION}/wasi-sdk-${WASI_SDK_VERSION}.0-x86_64-linux.tar.gz" /tmp/wasi-sdk.tar.gz
mkdir -p /opt/wasi-sdk
tar -xzf /tmp/wasi-sdk.tar.gz -C /opt/wasi-sdk --strip-components=1
rm /tmp/wasi-sdk.tar.gz

fetch "https://github.com/bytecodealliance/wasmtime/releases/download/${WASMTIME_VERSION}/wasmtime-${WASMTIME_VERSION}-x86_64-linux.tar.xz" /tmp/wasmtime.tar.xz
tar -xJf /tmp/wasmtime.tar.xz -C /tmp
mv "/tmp/wasmtime-${WASMTIME_VERSION}-x86_64-linux/wasmtime" /usr/local/bin/wasmtime
rm -rf /tmp/wasmtime.tar.xz "/tmp/wasmtime-${WASMTIME_VERSION}-x86_64-linux"

/opt/wasi-sdk/bin/clang --version
wasmtime --version
