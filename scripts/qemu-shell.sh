#!/usr/bin/env bash
# Build an interactive initrd and boot QEMU with a rash shell on the serial console.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export INITTAB_TEMPLATE="${INITTAB_TEMPLATE:-$ROOT/initrd/template/etc/inittab.shell}"
export INITRD_OUTPUT="${INITRD_OUTPUT:-$ROOT/initrd/initrd-shell.img}"

"$ROOT/scripts/mkinitrd.sh"

export INITRD="$INITRD_OUTPUT"
exec "$ROOT/scripts/qemu-test.sh"
