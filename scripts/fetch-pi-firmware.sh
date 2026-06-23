#!/usr/bin/env bash
# Download minimal Raspberry Pi 4 GPU firmware for the FAT /boot partition.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${PI_FIRMWARE_DIR:-$ROOT/kernel/pi-firmware}"
BASE_URL="${PI_FIRMWARE_URL:-https://raw.githubusercontent.com/raspberrypi/firmware/master/boot}"

log() {
    printf 'fetch-pi-firmware: %s\n' "$*" >&2
}

die() {
    printf 'fetch-pi-firmware: error: %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

fetch_one() {
    local rel=$1
    local out="$DEST/$rel"
    mkdir -p "$(dirname "$out")"
    if [[ -f "$out" && "${PI_FIRMWARE_FORCE:-0}" != "1" ]]; then
        return 0
    fi
    log "downloading $rel"
    curl -fsSL -o "$out" "$BASE_URL/$rel"
}

main() {
    need_cmd curl

    mkdir -p "$DEST"

    # Pi 4 EEPROM loads start4.elf; without these files the board will not boot.
    fetch_one start4.elf
    fetch_one fixup4.dat
    fetch_one LICENCE.broadcom

    log "firmware ready at $DEST"
}

main "$@"
