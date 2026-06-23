#!/usr/bin/env bash
# Shared QEMU helpers for rustbox test VMs.

# Default smoke-test wall-clock limit (seconds). Override with SMOKE_TIMEOUT.
SMOKE_TIMEOUT_SECS="${SMOKE_TIMEOUT_SECS:-60}"

# Append user-mode virtio-net unless QEMU_NET=0.
# Override with QEMU_NETDEV / QEMU_NET_DEVICE (see README.md).
append_qemu_net_args() {
    local -n _out=$1
    if [[ "${QEMU_NET:-1}" == "0" ]]; then
        return 0
    fi
    local netdev="${QEMU_NETDEV:-user,id=net0}"
    local device="${QEMU_NET_DEVICE:-virtio-net-pci,netdev=net0}"
    _out+=(-netdev "$netdev" -device "$device")
}
