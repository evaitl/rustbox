#!/usr/bin/env bash
# Build initrd and boot QEMU until the smoke test passes.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/qemu-common.sh
source "$ROOT/scripts/qemu-common.sh"
INITRD="${INITRD:-$ROOT/initrd/initrd.img}"
LOG="${SMOKE_LOG:-$ROOT/initrd/qemu-smoke.log}"
TIMEOUT="${SMOKE_TIMEOUT:-$SMOKE_TIMEOUT_SECS}"
: "${TIMEOUT:=30}"

die() {
    printf 'qemu-smoke: error: %s\n' "$*" >&2
    exit 1
}

kill_qemu_force() {
    local pid=$1
    [[ -n "$pid" ]] || return 0
    kill -KILL "$pid" 2>/dev/null || true
    pkill -KILL -P "$pid" 2>/dev/null || true
    pkill -KILL -f "qemu-system-x86_64.*${INITRD}" 2>/dev/null || true
}

main() {
    command -v qemu-system-x86_64 >/dev/null 2>&1 || die "missing qemu-system-x86_64"

    "$ROOT/scripts/mkinitrd.sh"

    local kernel=""
    if [[ -n "${KERNEL:-}" && -f "${KERNEL}" ]]; then
        kernel="${KERNEL}"
    elif [[ -f "$ROOT/kernel/vmlinuz" ]]; then
        kernel="$ROOT/kernel/vmlinuz"
    elif [[ -f "/boot/vmlinuz-$(uname -r)" ]]; then
        kernel="/boot/vmlinuz-$(uname -r)"
    else
        printf 'qemu-smoke: kernel/vmlinuz missing; run build-kernel.sh first\n' >&2
        "$ROOT/scripts/build-kernel.sh"
        kernel="$ROOT/kernel/vmlinuz"
    fi

    printf 'qemu-smoke: kernel=%s\n' "$kernel" >&2
    printf 'qemu-smoke: initrd=%s\n' "$INITRD" >&2
    printf 'qemu-smoke: timeout=%ss\n' "$TIMEOUT" >&2

    rm -f "$LOG"
    local qemu_args=(
        -kernel "$kernel"
        -initrd "$INITRD"
        -append "console=ttyS0 rdinit=/init panic=1 init=/init"
        -m "${MEMORY:-256M}"
        -nographic
        -no-reboot
    )
    append_qemu_net_args qemu_args
    local deadline=$((SECONDS + TIMEOUT))
    local qemu_status=0
    local timed_out=0

    if command -v stdbuf >/dev/null 2>&1; then
        qemu-system-x86_64 "${qemu_args[@]}" > >(stdbuf -oL tee "$LOG") 2>&1 &
    else
        qemu-system-x86_64 "${qemu_args[@]}" > >(tee "$LOG") 2>&1 &
    fi
    local qemu_pid=$!

    set +e
    while kill -0 "$qemu_pid" 2>/dev/null; do
        if [[ -f "$LOG" ]] && grep -q 'smoke: ok' "$LOG"; then
            kill_qemu_force "$qemu_pid"
            wait "$qemu_pid" 2>/dev/null || true
            break
        fi
        if [[ -f "$LOG" ]] && grep -q 'smoke: timeout' "$LOG"; then
            kill_qemu_force "$qemu_pid"
            wait "$qemu_pid" 2>/dev/null || true
            break
        fi
        if (( SECONDS >= deadline )); then
            kill_qemu_force "$qemu_pid"
            wait "$qemu_pid" 2>/dev/null || true
            timed_out=1
            qemu_status=124
            break
        fi
        sleep 0.2
    done
    if kill -0 "$qemu_pid" 2>/dev/null; then
        wait "$qemu_pid"
        qemu_status=$?
    elif [[ "$timed_out" -eq 0 ]]; then
        wait "$qemu_pid" 2>/dev/null || true
        qemu_status=$?
    fi
    set -e

    if grep -q 'smoke: ok' "$LOG"; then
        printf 'qemu-smoke: passed\n' >&2
        exit 0
    fi

    if grep -q 'smoke: timeout' "$LOG"; then
        die "smoke test exceeded ${TIMEOUT}s deadline (see $LOG)"
    fi

    if [[ "$qemu_status" -eq 124 ]]; then
        die "timed out after ${TIMEOUT}s waiting for smoke: ok"
    fi

    die "smoke test failed or missing smoke: ok (see $LOG)"
}

main "$@"
