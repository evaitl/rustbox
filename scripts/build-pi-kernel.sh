#!/usr/bin/env bash
# Build an arm64 Raspberry Pi 4 kernel Image and DTB from the Raspberry Pi Linux tree.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KERNEL_SRC="${KERNEL_SRC:-}"
KERNEL_CONFIG="${KERNEL_CONFIG:-$ROOT/kernel/config.pi4}"
INSTALL_DIR="${KERNEL_INSTALL_DIR:-$ROOT/kernel/pi}"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 1)}"
ARCH=arm64
CROSS_COMPILE="${CROSS_COMPILE:-aarch64-linux-gnu-}"
TOOLS="${KERNEL_TOOLS:-$ROOT/kernel/.tools}"

die() {
    printf 'build-pi-kernel: error: %s\n' "$*" >&2
    exit 1
}

need_cross_compiler() {
    if command -v "${CROSS_COMPILE}gcc" >/dev/null 2>&1; then
        return
    fi
    die "missing ${CROSS_COMPILE}gcc (install gcc-aarch64-linux-gnu)"
}

setup_tools() {
    if [[ ! -x "$TOOLS/usr/bin/flex" && -x "$ROOT/scripts/setup-kernel-tools.sh" ]]; then
        if ! command -v flex >/dev/null || ! command -v bison >/dev/null || ! command -v m4 >/dev/null; then
            "$ROOT/scripts/setup-kernel-tools.sh"
        fi
    fi
    if [[ -x "$TOOLS/usr/bin/flex" ]]; then
        export PATH="$TOOLS/usr/bin:$PATH"
        export M4="$TOOLS/usr/bin/m4"
        export BISON_PKGDATADIR="$TOOLS/usr/share/bison"
        if [[ ! -f /usr/include/openssl/ssl.h && -f "$TOOLS/usr/include/openssl/ssl.h" ]]; then
            export C_INCLUDE_PATH="$TOOLS/usr/include${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"
        fi
        if [[ ! -f /usr/include/libelf.h && -f "$TOOLS/usr/include/libelf.h" ]]; then
            export C_INCLUDE_PATH="$TOOLS/usr/include${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"
        fi
    fi
}

find_kernel_src() {
    if [[ -n "$KERNEL_SRC" ]]; then
        printf '%s\n' "$KERNEL_SRC"
        return
    fi
    if [[ -d "$ROOT/kernel/linux-rpi" ]]; then
        printf '%s\n' "$ROOT/kernel/linux-rpi"
        return
    fi
    die "set KERNEL_SRC to a Raspberry Pi Linux tree (see docs/RASPBERRY4.md)"
}

merge_fragment() {
    local src=$1
    local fragment=$2
    if [[ ! -f "$fragment" ]]; then
        return
    fi
    if [[ -x "$src/scripts/kconfig/merge_config.sh" ]]; then
        (cd "$src" && ./scripts/kconfig/merge_config.sh -m .config "$fragment")
        make -C "$src" ARCH="$ARCH" CROSS_COMPILE="$CROSS_COMPILE" olddefconfig
        return
    fi
    printf 'build-pi-kernel: warning: merge_config.sh not found; skipping %s\n' "$fragment" >&2
}

main() {
    setup_tools
    need_cross_compiler

    KERNEL_SRC="$(find_kernel_src)"
    [[ -f "$KERNEL_SRC/Makefile" ]] || die "KERNEL_SRC is not a kernel tree: $KERNEL_SRC"

    printf 'build-pi-kernel: source=%s\n' "$KERNEL_SRC" >&2
    printf 'build-pi-kernel: cross=%s\n' "$CROSS_COMPILE" >&2

    make -C "$KERNEL_SRC" ARCH="$ARCH" CROSS_COMPILE="$CROSS_COMPILE" bcm2711_defconfig
    merge_fragment "$KERNEL_SRC" "$KERNEL_CONFIG"
    make -C "$KERNEL_SRC" ARCH="$ARCH" CROSS_COMPILE="$CROSS_COMPILE" -j"$JOBS" Image dtbs

    local image="$KERNEL_SRC/arch/arm64/boot/Image"
    local dtb="$KERNEL_SRC/arch/arm64/boot/dts/broadcom/bcm2711-rpi-4-b.dtb"
    [[ -f "$image" ]] || die "kernel Image not found after build"
    [[ -f "$dtb" ]] || die "DTB not found after build: $dtb"

    install -D "$image" "$INSTALL_DIR/Image"
    install -D "$dtb" "$INSTALL_DIR/bcm2711-rpi-4-b.dtb"
    printf 'build-pi-kernel: wrote %s/Image\n' "$INSTALL_DIR"
    printf 'build-pi-kernel: wrote %s/bcm2711-rpi-4-b.dtb\n' "$INSTALL_DIR"
    printf 'build-pi-kernel: next: ./scripts/mk-pi-image.sh\n'
}

main "$@"
