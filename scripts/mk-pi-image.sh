#!/usr/bin/env bash
# Assemble a Raspberry Pi 4 SD card image (FAT boot + ext4 root) from staged rootfs and kernel/pi.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGING="${PI_ROOTFS_STAGING:-$ROOT/pi4/staging}"
IMAGE="${PI_IMAGE:-$ROOT/pi4/sdcard.img}"
IMAGE_SIZE="${PI_IMAGE_SIZE:-2G}"
BOOT_SIZE_MB="${PI_BOOT_SIZE_MB:-128}"
KERNEL_DIR="${PI_KERNEL_DIR:-$ROOT/kernel/pi}"
BOOT_TEMPLATE="$ROOT/pi4/template/boot"
FIRMWARE_DIR="${PI_FIRMWARE_DIR:-$ROOT/kernel/pi-firmware}"

log() {
    printf 'mk-pi-image: %s\n' "$*" >&2
}

die() {
    printf 'mk-pi-image: error: %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

need_root() {
    if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
        die "run as root (sudo ./scripts/mk-pi-image.sh) to partition and format the image"
    fi
}

ensure_staging() {
    if [[ ! -f "$STAGING/bin/rustbox" ]]; then
        if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
            die "run ./scripts/mk-pi-rootfs.sh as your normal user first, then sudo ./scripts/mk-pi-image.sh"
        fi
        log "staging missing; running mk-pi-rootfs.sh"
        "$ROOT/scripts/mk-pi-rootfs.sh"
    fi
    [[ -f "$STAGING/bin/rustbox" ]] || die "rootfs staging not found at $STAGING"
}

ensure_kernel() {
    [[ -f "$KERNEL_DIR/Image" ]] || die "kernel Image missing at $KERNEL_DIR/Image (run ./scripts/build-pi-kernel.sh)"
    [[ -f "$KERNEL_DIR/bcm2711-rpi-4-b.dtb" ]] || die "DTB missing at $KERNEL_DIR/bcm2711-rpi-4-b.dtb"
}

ensure_firmware() {
    if [[ ! -f "$FIRMWARE_DIR/start4.elf" ]]; then
        log "Pi GPU firmware missing; fetching"
        "$ROOT/scripts/fetch-pi-firmware.sh"
    fi
    [[ -f "$FIRMWARE_DIR/start4.elf" ]] || die "start4.elf missing at $FIRMWARE_DIR (run ./scripts/fetch-pi-firmware.sh)"
    [[ -f "$FIRMWARE_DIR/fixup4.dat" ]] || die "fixup4.dat missing at $FIRMWARE_DIR"
}

create_partition_table() {
    rm -f "$IMAGE"
    truncate -s "$IMAGE_SIZE" "$IMAGE"
    parted -s "$IMAGE" mklabel msdos
    parted -s "$IMAGE" mkpart primary fat32 1MiB "${BOOT_SIZE_MB}MiB"
    parted -s "$IMAGE" mkpart primary ext4 "${BOOT_SIZE_MB}MiB" 100%
    parted -s "$IMAGE" set 1 boot on
}

populate_image() {
    local loop boot_mnt root_mnt
    loop="$(losetup -f --show -P "$IMAGE")"
    cleanup() {
        mountpoint -q "$boot_mnt" 2>/dev/null && umount "$boot_mnt"
        mountpoint -q "$root_mnt" 2>/dev/null && umount "$root_mnt"
        [[ -n "${loop:-}" ]] && losetup -d "$loop" 2>/dev/null || true
        rmdir "$boot_mnt" "$root_mnt" 2>/dev/null || true
    }
    trap cleanup EXIT

    mkfs.vfat -F 32 -n BOOT "${loop}p1"
    mkfs.ext4 -F -L rootfs "${loop}p2"

    boot_mnt="$(mktemp -d)"
    root_mnt="$(mktemp -d)"
    mount "${loop}p1" "$boot_mnt"
    mount "${loop}p2" "$root_mnt"

    install -m 0644 "$KERNEL_DIR/Image" "$boot_mnt/kernel8.img"
    install -m 0644 "$KERNEL_DIR/bcm2711-rpi-4-b.dtb" "$boot_mnt/bcm2711-rpi-4-b.dtb"
    install -m 0644 "$FIRMWARE_DIR/start4.elf" "$boot_mnt/start4.elf"
    install -m 0644 "$FIRMWARE_DIR/fixup4.dat" "$boot_mnt/fixup4.dat"
    if [[ -f "$FIRMWARE_DIR/LICENCE.broadcom" ]]; then
        install -m 0644 "$FIRMWARE_DIR/LICENCE.broadcom" "$boot_mnt/LICENCE.broadcom"
    fi
    install -m 0644 "$BOOT_TEMPLATE/config.txt" "$boot_mnt/config.txt"
    install -m 0644 "$BOOT_TEMPLATE/cmdline.txt" "$boot_mnt/cmdline.txt"

    cp -a "$STAGING/." "$root_mnt/"
    sync
}

main() {
    need_cmd parted
    need_cmd truncate
    need_cmd losetup
    need_cmd mkfs.vfat
    need_cmd mkfs.ext4
    need_cmd mount
    need_cmd umount
    need_root

    ensure_staging
    ensure_kernel
    ensure_firmware
    create_partition_table
    populate_image

    mkdir -p "$(dirname "$IMAGE")"
    log "wrote $IMAGE ($(du -h "$IMAGE" | awk '{print $1}'))"
    log "hardware: flash with dd; console on HDMI tty1 + USB keyboard"
    log "flash: sudo dd if=$IMAGE of=/dev/sdX bs=4M conv=fsync status=progress"
}

main "$@"
