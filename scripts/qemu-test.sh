#!/usr/bin/env bash
# Boot the rustbox initrd in QEMU.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/qemu-common.sh
source "$ROOT/scripts/qemu-common.sh"
INITRD="${INITRD:-$ROOT/initrd/initrd.img}"
KERNEL="${KERNEL:-}"
APPEND="${APPEND:-console=ttyS0 rdinit=/init panic=1 init=/init}"
MEMORY="${MEMORY:-256M}"

die() {
    printf 'qemu-test: error: %s\n' "$*" >&2
    exit 1
}

find_kernel() {
    if [[ -n "$KERNEL" && -f "$KERNEL" ]]; then
        printf '%s\n' "$KERNEL"
        return
    fi

    local candidates=(
        "$ROOT/kernel/vmlinuz"
        "/boot/vmlinuz-$(uname -r)"
        "/boot/vmlinuz"
        "/lib/modules/$(uname -r)/vmlinuz"
    )
    for path in "${candidates[@]}"; do
        if [[ -f "$path" ]]; then
            printf '%s\n' "$path"
            return
        fi
    done

    die "kernel not found; set KERNEL=/path/to/vmlinuz"
}

main() {
    command -v qemu-system-x86_64 >/dev/null 2>&1 || die "missing qemu-system-x86_64"
    [[ -f "$INITRD" ]] || die "initrd not found at $INITRD (run ./scripts/mkinitrd.sh first)"

    local kernel
    kernel="$(find_kernel)"

    printf 'qemu-test: kernel=%s\n' "$kernel"
    printf 'qemu-test: initrd=%s\n' "$INITRD"
    printf 'qemu-test: append=%s\n' "$APPEND"
    if [[ "${QEMU_NET:-1}" != "0" ]]; then
        printf 'qemu-test: netdev=%s device=%s\n' \
            "${QEMU_NETDEV:-user,id=net0}" \
            "${QEMU_NET_DEVICE:-virtio-net-pci,netdev=net0}"
    else
        printf 'qemu-test: networking disabled (QEMU_NET=0)\n'
    fi

    local qemu_args=(
        -kernel "$kernel"
        -initrd "$INITRD"
        -append "$APPEND"
        -m "$MEMORY"
        -nographic
        -no-reboot
    )
    append_qemu_net_args qemu_args
    exec qemu-system-x86_64 "${qemu_args[@]}"
}

main "$@"
