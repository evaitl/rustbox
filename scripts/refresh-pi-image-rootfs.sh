#!/usr/bin/env bash
# Rewrite only the ext4 root partition in an existing Pi SD image (no sudo).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGING="${PI_ROOTFS_STAGING:-$ROOT/pi4/staging}"
IMAGE="${PI_IMAGE:-$ROOT/pi4/sdcard.img}"
BOOT_SIZE_MB="${PI_BOOT_SIZE_MB:-128}"
IMAGE_SIZE="${PI_IMAGE_SIZE:-2G}"

log() {
    printf 'refresh-pi-image-rootfs: %s\n' "$*" >&2
}

die() {
    printf 'refresh-pi-image-rootfs: error: %s\n' "$*" >&2
    exit 1
}

image_size_bytes() {
    local size="$1"
    case "$size" in
        *K|*k) echo $(( ${size%[Kk]} * 1024 )) ;;
        *M|*m) echo $(( ${size%[Mm]} * 1024 * 1024 )) ;;
        *G|*g) echo $(( ${size%[Gg]} * 1024 * 1024 * 1024 )) ;;
        *) die "unsupported PI_IMAGE_SIZE: $size" ;;
    esac
}

main() {
    command -v mkfs.ext4 >/dev/null 2>&1 || die "missing mkfs.ext4"
    [[ -f "$STAGING/bin/rustbox" ]] || die "staging not found at $STAGING (run ./scripts/mk-pi-rootfs.sh)"
    [[ -f "$IMAGE" ]] || die "image not found at $IMAGE (run ./scripts/mk-pi-image.sh)"

    local total_bytes root_offset_bytes root_size_mb root_img
    total_bytes="$(image_size_bytes "$IMAGE_SIZE")"
    root_offset_bytes=$((BOOT_SIZE_MB * 1024 * 1024))
    root_size_mb=$(( (total_bytes - root_offset_bytes) / 1024 / 1024 ))

    root_img="$(mktemp)"
    log "building ext4 root (${root_size_mb}MiB) from $STAGING"
    mkfs.ext4 -F -L rootfs -d "$STAGING" "$root_img" "${root_size_mb}M"

    log "writing root partition into $IMAGE at offset ${root_offset_bytes}B"
    dd if="$root_img" of="$IMAGE" bs=512 seek=$((root_offset_bytes / 512)) conv=notrunc status=none
    rm -f "$root_img"

    log "done"
}

main "$@"
