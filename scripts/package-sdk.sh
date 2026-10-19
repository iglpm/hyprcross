#!/usr/bin/env bash
# package-sdk.sh - Package native SDK sysroots for hyprcross into .tar.xz archives
#
# Usage:
#   ./scripts/package-sdk.sh from-image <target> [image_tag] [output_dir]
#   ./scripts/package-sdk.sh from-dir <target> <dir_path> [output_dir]
#   ./scripts/package-sdk.sh all-images [output_dir]
#
# Output:
#   Produces <output_dir>/<target>.tar.xz suitable for publishing to GitHub Releases (e.g. runepm/sdks).

set -euo pipefail

OUT_DIR="${OUT_DIR:-dist/sdks}"
CONTAINER_TOOL=""

detect_container_tool() {
    if command -v podman >/dev/null 2>&1; then
        CONTAINER_TOOL="podman"
    elif command -v docker >/dev/null 2>&1; then
        CONTAINER_TOOL="docker"
    else
        echo "Error: Neither podman nor docker was found on PATH." >&2
        exit 1
    fi
}

package_from_dir() {
    local target="$1"
    local src_dir="$2"
    local dest_dir="${3:-$OUT_DIR}"

    if [ ! -d "$src_dir" ]; then
        echo "Error: Source directory '$src_dir' does not exist." >&2
        exit 1
    fi

    mkdir -p "$dest_dir"
    local out_tar="$dest_dir/${target}.tar.xz"
    echo "==> Packaging $target from $src_dir to $out_tar ..."

    # Use max compression with xz
    tar -C "$src_dir" -cJf "$out_tar" .
    echo "✓ Created $out_tar ($(du -h "$out_tar" | cut -f1))"
}

package_from_image() {
    local target="$1"
    local image="${2:-ghcr.io/runepm/hyprcross:$target}"
    local dest_dir="${3:-$OUT_DIR}"

    detect_container_tool
    mkdir -p "$dest_dir"
    local work_dir
    work_dir="$(mktemp -d)"
    trap 'rm -rf "$work_dir"' EXIT

    echo "==> Extracting sysroot for $target from $image using $CONTAINER_TOOL..."

    # Create a container without starting it
    local cid
    cid="$($CONTAINER_TOOL create "$image" 2>/dev/null || true)"
    if [ -z "$cid" ]; then
        echo "Error: Failed to create container from $image. Ensure image exists or pull it first." >&2
        exit 1
    fi

    # Target specific sysroot locations in our images
    local extract_root="$work_dir/sysroot"
    mkdir -p "$extract_root/usr/lib" "$extract_root/usr/include" "$extract_root/lib"

    # Try copying target multiarch sysroot or standard sysroot paths
    case "$target" in
        aarch64-unknown-linux-gnu)
            $CONTAINER_TOOL cp "$cid:/usr/aarch64-linux-gnu/." "$extract_root/usr/" 2>/dev/null || true
            $CONTAINER_TOOL cp "$cid:/usr/lib/aarch64-linux-gnu/." "$extract_root/usr/lib/" 2>/dev/null || true
            ;;
        armv7-unknown-linux-gnueabihf)
            $CONTAINER_TOOL cp "$cid:/usr/arm-linux-gnueabihf/." "$extract_root/usr/" 2>/dev/null || true
            $CONTAINER_TOOL cp "$cid:/usr/lib/arm-linux-gnueabihf/." "$extract_root/usr/lib/" 2>/dev/null || true
            ;;
        riscv64gc-unknown-linux-gnu)
            $CONTAINER_TOOL cp "$cid:/usr/riscv64-linux-gnu/." "$extract_root/usr/" 2>/dev/null || true
            $CONTAINER_TOOL cp "$cid:/usr/lib/riscv64-linux-gnu/." "$extract_root/usr/lib/" 2>/dev/null || true
            ;;
        x86_64-unknown-linux-gnu)
            $CONTAINER_TOOL cp "$cid:/usr/lib/x86_64-linux-gnu/." "$extract_root/usr/lib/" 2>/dev/null || true
            $CONTAINER_TOOL cp "$cid:/usr/include/x86_64-linux-gnu/." "$extract_root/usr/include/" 2>/dev/null || true
            $CONTAINER_TOOL cp "$cid:/lib/x86_64-linux-gnu/." "$extract_root/lib/" 2>/dev/null || true
            ;;
        *-linux-musl)
            local musl_prefix="/usr/local/musl"
            $CONTAINER_TOOL cp "$cid:$musl_prefix/." "$extract_root/usr/" 2>/dev/null || true
            ;;
        *-windows-gnu)
            $CONTAINER_TOOL cp "$cid:/usr/x86_64-w64-mingw32/." "$extract_root/usr/" 2>/dev/null || \
            $CONTAINER_TOOL cp "$cid:/usr/i686-w64-mingw32/." "$extract_root/usr/" 2>/dev/null || true
            ;;
        *-freebsd)
            $CONTAINER_TOOL cp "$cid:/usr/local/freebsd/." "$extract_root/usr/" 2>/dev/null || \
            $CONTAINER_TOOL cp "$cid:/usr/freebsd/." "$extract_root/usr/" 2>/dev/null || true
            ;;
        *)
            # Fallback copy
            $CONTAINER_TOOL cp "$cid:/usr/lib/." "$extract_root/usr/lib/" 2>/dev/null || true
            $CONTAINER_TOOL cp "$cid:/usr/include/." "$extract_root/usr/include/" 2>/dev/null || true
            ;;
    esac

    # Remove the container
    $CONTAINER_TOOL rm -f "$cid" >/dev/null 2>&1 || true

    # Strip unnecessary heavy items like static archives or docs if present
    find "$extract_root" -type f -name "*.a" -not -name "libc.a" -not -name "libm.a" -not -name "libpthread.a" -delete 2>/dev/null || true
    find "$extract_root" -type d -name "doc" -o -name "man" -o -name "locale" | xargs rm -rf 2>/dev/null || true

    package_from_dir "$target" "$extract_root" "$dest_dir"
}

package_all_images() {
    local dest_dir="${1:-$OUT_DIR}"
    local targets=(
        "aarch64-unknown-linux-gnu"
        "armv7-unknown-linux-gnueabihf"
        "arm-unknown-linux-gnueabi"
        "i686-unknown-linux-gnu"
        "powerpc64le-unknown-linux-gnu"
        "riscv64gc-unknown-linux-gnu"
        "s390x-unknown-linux-gnu"
        "x86_64-unknown-linux-gnu"
        "aarch64-unknown-linux-musl"
        "x86_64-unknown-linux-musl"
        "x86_64-pc-windows-gnu"
        "x86_64-unknown-freebsd"
    )

    for target in "${targets[@]}"; do
        echo "=================================================="
        echo "Processing $target..."
        package_from_image "$target" "ghcr.io/runepm/hyprcross:$target" "$dest_dir" || {
            echo "Warning: Failed to package $target, continuing..." >&2
        }
    done
}

# CLI routing
command="${1:-help}"
case "$command" in
    from-image)
        if [ -z "${2:-}" ]; then
            echo "Usage: $0 from-image <target> [image_tag] [output_dir]" >&2
            exit 1
        fi
        package_from_image "$2" "${3:-}" "${4:-}"
        ;;
    from-dir)
        if [ -z "${2:-}" ] || [ -z "${3:-}" ]; then
            echo "Usage: $0 from-dir <target> <dir_path> [output_dir]" >&2
            exit 1
        fi
        package_from_dir "$2" "$3" "${4:-}"
        ;;
    all-images)
        package_all_images "${2:-}"
        ;;
    help|--help|-h)
        echo "hyprcross SDK Packaging Tool"
        echo ""
        echo "Commands:"
        echo "  from-image <target> [image] [out]  Extract & package SDK from Docker/Podman image"
        echo "  from-dir <target> <path> [out]     Package SDK from a local directory"
        echo "  all-images [out]                   Package all known primary images"
        echo ""
        echo "Default output directory: $OUT_DIR"
        ;;
    *)
        echo "Unknown command: $command. Run with --help for usage." >&2
        exit 1
        ;;
esac
