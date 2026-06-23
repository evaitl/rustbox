#!/usr/bin/env bash
# Build a Linux bzImage from kernel/config.qemu for QEMU rustbox testing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KERNEL_SRC="${KERNEL_SRC:-}"
KERNEL_CONFIG="${KERNEL_CONFIG:-$ROOT/kernel/config.qemu}"
INSTALL="${KERNEL_INSTALL:-$ROOT/kernel/vmlinuz}"
JOBS="${JOBS:-$(nproc 2>/dev/null || echo 1)}"
TOOLS="${KERNEL_TOOLS:-$ROOT/kernel/.tools}"

die() {
    printf 'build-kernel: error: %s\n' "$*" >&2
    exit 1
}

find_kernel_src() {
    if [[ -n "$KERNEL_SRC" ]]; then
        printf '%s\n' "$KERNEL_SRC"
        return
    fi
    die "set KERNEL_SRC to a Linux source tree outside this repository (see docs/QEMU.md)"
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

main() {
    setup_tools
    KERNEL_SRC="$(find_kernel_src)"
    [[ -f "$KERNEL_SRC/Makefile" ]] || die "KERNEL_SRC is not a kernel tree: $KERNEL_SRC"
    [[ -f "$KERNEL_CONFIG" ]] || die "missing config: $KERNEL_CONFIG"

    printf 'build-kernel: source=%s\n' "$KERNEL_SRC" >&2
    install -D "$KERNEL_CONFIG" "$KERNEL_SRC/.config"
    make -C "$KERNEL_SRC" ARCH=x86_64 olddefconfig
    make -C "$KERNEL_SRC" ARCH=x86_64 -j"$JOBS"

    install -D "$KERNEL_SRC/arch/x86/boot/bzImage" "$INSTALL"
    printf 'build-kernel: wrote %s\n' "$INSTALL"
    printf 'build-kernel: run: KERNEL=%s ./scripts/qemu-test.sh\n' "$INSTALL"
}

main "$@"
