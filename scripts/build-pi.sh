#!/usr/bin/env bash
# Build a flashable Raspberry Pi 4 SD image for real hardware.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/pi-musl-env.sh
source "$ROOT/scripts/pi-musl-env.sh"

log() {
    printf 'build-pi: %s\n' "$*" >&2
}

die() {
    printf 'build-pi: error: %s\n' "$*" >&2
    exit 1
}

main() {
    export KERNEL_SRC="${KERNEL_SRC:-$ROOT/kernel/linux-rpi}"
    unset PI_INITTAB

    if [[ "${PI_SKIP_FIRMWARE:-0}" != "1" ]]; then
        "$ROOT/scripts/fetch-pi-firmware.sh"
    fi

    if [[ "${PI_SKIP_KERNEL:-0}" != "1" ]]; then
        if [[ ! -f "$ROOT/kernel/pi/Image" || "${PI_FORCE_KERNEL:-0}" == "1" ]]; then
            if [[ ! -d "$KERNEL_SRC" ]]; then
                die "kernel source not found at $KERNEL_SRC (clone raspberrypi/linux to kernel/linux-rpi or set KERNEL_SRC)"
            fi
            "$ROOT/scripts/build-pi-kernel.sh"
        else
            log "reusing kernel/pi (set PI_FORCE_KERNEL=1 to rebuild)"
        fi
    fi

    setup_pi_musl_env || die "aarch64 musl cross linker not available"
    "$ROOT/scripts/mk-pi-rootfs.sh"

    if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
        "$ROOT/scripts/mk-pi-image.sh"
    elif [[ -f "${PI_IMAGE:-$ROOT/pi4/sdcard.img}" ]]; then
        log "updating root partition in existing image (no sudo)"
        "$ROOT/scripts/refresh-pi-image-rootfs.sh"
        log "note: boot partition unchanged; run sudo ./scripts/mk-pi-image.sh for a fresh image"
    else
        die "run: sudo ./scripts/mk-pi-image.sh  (first image needs root for partitioning)"
    fi

    log "flash: sudo dd if=${PI_IMAGE:-$ROOT/pi4/sdcard.img} of=/dev/sdX bs=4M conv=fsync status=progress"
    log "console: HDMI monitor + USB keyboard on tty1"
}

main "$@"
