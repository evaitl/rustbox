#!/usr/bin/env bash
# Long-running QEMU soak: boot initrd, exercise daemons, monitor for hangs and memory growth.
#
# Defaults: 12h run, check every 15 minutes.
#
# Environment:
#   SOAK_DURATION       Total run time in seconds (default: 43200 = 12h)
#   CHECK_INTERVAL      Hang/memory check period in seconds (default: 900 = 15m)
#   BOOT_TIMEOUT        Wait for smoke: ok (default: 600)
#   HEARTBEAT_MAX_AGE   Fail if no soak heartbeat within this many seconds (default: CHECK_INTERVAL + 120)
#   LEAK_STEP_KB        Per-check RSS growth that counts toward leak detection (default: 2048)
#   LEAK_FAIL_CONSEC    Consecutive growing checks before failing (default: 3)
#   LEAK_TOTAL_KB       Fail if mem_used_kb exceeds baseline by this much (default: 32768)
#   SOAK_LOG            Serial log path (default: initrd/qemu-soak.log)
#   SOAK_METRICS        Host metrics log (default: initrd/qemu-soak.metrics)
#   INITRD, KERNEL, MEMORY, QEMU_NET — same as qemu-smoke.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/qemu-common.sh
source "$ROOT/scripts/qemu-common.sh"

INITRD="${INITRD:-$ROOT/initrd/initrd.img}"
LOG="${SOAK_LOG:-$ROOT/initrd/qemu-soak.log}"
METRICS="${SOAK_METRICS:-$ROOT/initrd/qemu-soak.metrics}"
WATCHDOG="${SOAK_WATCHDOG:-$ROOT/initrd/qemu-soak.watchdog}"

SOAK_DURATION="${SOAK_DURATION:-$((12 * 3600))}"
CHECK_INTERVAL="${CHECK_INTERVAL:-900}"
BOOT_TIMEOUT="${BOOT_TIMEOUT:-600}"
HEARTBEAT_GRACE_SEC="${HEARTBEAT_GRACE_SEC:-180}"
LEAK_STEP_KB="${LEAK_STEP_KB:-2048}"
LEAK_FAIL_CONSEC="${LEAK_FAIL_CONSEC:-3}"
LEAK_TOTAL_KB="${LEAK_TOTAL_KB:-32768}"

HEARTBEAT_MAX_AGE="${HEARTBEAT_MAX_AGE:-$((CHECK_INTERVAL + 120))}"

qemu_pid=""
fail_reason=""
last_heartbeat_count=0

die() {
    printf 'qemu-soak: error: %s\n' "$*" >&2
    fail_reason="$*"
    exit 1
}

log_metric() {
    printf '%s %s\n' "$(date -Is)" "$*" | tee -a "$METRICS" >&2
}

kill_qemu_force() {
    local pid=$1
    [[ -n "$pid" ]] || return 0
    kill -KILL "$pid" 2>/dev/null || true
    pkill -KILL -P "$pid" 2>/dev/null || true
    pkill -KILL -f "qemu-system-x86_64.*${INITRD}" 2>/dev/null || true
}

cleanup() {
    local status=$?
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill_qemu_force "$qemu_pid"
        wait "$qemu_pid" 2>/dev/null || true
    fi
    if [[ "$status" -ne 0 && -n "$fail_reason" ]]; then
        log_metric "FAIL: $fail_reason"
    fi
}

trap cleanup EXIT

resolve_soak_kernel() {
    if [[ -n "${KERNEL:-}" && -f "${KERNEL}" ]]; then
        printf '%s\n' "$KERNEL"
        return 0
    fi
    if [[ -f "$ROOT/kernel/vmlinuz" ]]; then
        printf '%s\n' "$ROOT/kernel/vmlinuz"
        return 0
    fi
    printf 'qemu-soak: kernel/vmlinuz missing; building from kernel/config.qemu\n' >&2
    "$ROOT/scripts/build-kernel.sh" >&2
    [[ -f "$ROOT/kernel/vmlinuz" ]] || die "build-kernel.sh did not produce kernel/vmlinuz"
    printf '%s\n' "$ROOT/kernel/vmlinuz"
}

last_heartbeat_line() {
    grep -a 'soak: heartbeat' "$LOG" 2>/dev/null | tail -1 || true
}

heartbeat_count() {
    grep -ac 'soak: heartbeat' "$LOG" 2>/dev/null || echo 0
}

parse_heartbeat_field() {
    local line=$1 field=$2
  [[ -n "$line" ]] || return 1
    sed -n "s/.*[[:space:]]${field}=\\([^[:space:]]*\\).*/\\1/p" <<<"$line" | head -1
}

host_free_kb() {
    awk '/^Mem:/ {print $3}' <(free -k 2>/dev/null || free)
}

wait_for_smoke_ok() {
    local deadline=$((SECONDS + BOOT_TIMEOUT))
    while kill -0 "$qemu_pid" 2>/dev/null; do
        if [[ -f "$LOG" ]] && grep -q 'smoke: ok' "$LOG"; then
            log_metric "boot: smoke: ok"
            return 0
        fi
        if [[ -f "$LOG" ]] && grep -q 'smoke: timeout' "$LOG"; then
            die "guest smoke test timed out (see $LOG)"
        fi
        if (( SECONDS >= deadline )); then
            die "timed out after ${BOOT_TIMEOUT}s waiting for smoke: ok"
        fi
        sleep 1
    done
    die "QEMU exited before smoke: ok"
}

wait_for_first_heartbeat() {
    local deadline=$((SECONDS + HEARTBEAT_GRACE_SEC))
    while kill -0 "$qemu_pid" 2>/dev/null; do
        if [[ -n "$(last_heartbeat_line)" ]]; then
            log_metric "boot: first soak heartbeat"
            return 0
        fi
        if (( SECONDS >= deadline )); then
            die "no soak heartbeat within ${HEARTBEAT_GRACE_SEC}s after smoke: ok"
        fi
        sleep 2
    done
    die "QEMU exited before first soak heartbeat"
}

check_hang() {
    local line epoch now age count
    count=$(heartbeat_count)
    if [[ "$count" -le "$last_heartbeat_count" ]]; then
        die "hang: no new soak heartbeats since last check (count=${count})"
    fi
    last_heartbeat_count=$count

    line="$(last_heartbeat_line)"
    if [[ -z "$line" ]]; then
        die "no soak: heartbeat lines in log yet"
    fi
    epoch="$(parse_heartbeat_field "$line" epoch)"
    [[ -n "$epoch" ]] || die "could not parse heartbeat epoch from: $line"
    now=$(date +%s)
    age=$((now - epoch))
    if (( age > HEARTBEAT_MAX_AGE )); then
        die "hang: last heartbeat ${age}s ago (max ${HEARTBEAT_MAX_AGE}s)"
    fi
    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        die "QEMU process exited"
    fi
}

check_memory_leak() {
    local line mem thttpd dnscached syslogd
    local -n _baseline_mem=$1
    local -n _baseline_thttpd=$2
    local -n _baseline_dns=$3
    local -n _baseline_syslog=$4
    local -n _prev_mem=$5
    local -n _leak_consec=$6

    line="$(last_heartbeat_line)"
    [[ -n "$line" ]] || return 0

    mem="$(parse_heartbeat_field "$line" mem_used_kb)"
    thttpd="$(parse_heartbeat_field "$line" thttpd_kb)"
    dnscached="$(parse_heartbeat_field "$line" dnscached_kb)"
    syslogd="$(parse_heartbeat_field "$line" syslogd_kb)"
    [[ -n "$mem" ]] || die "could not parse mem_used_kb from heartbeat"

    if [[ -z "$_baseline_mem" ]]; then
        _baseline_mem=$mem
        _baseline_thttpd=${thttpd:-0}
        _baseline_dns=${dnscached:-0}
        _baseline_syslog=${syslogd:-0}
        _prev_mem=$mem
        log_metric "mem-baseline guest mem_used_kb=${_baseline_mem} thttpd_kb=${_baseline_thttpd} dnscached_kb=${_baseline_dns} syslogd_kb=${_baseline_syslog} host_mem_used_kb=$(host_free_kb)"
        return 0
    fi

    local delta=$((mem - _baseline_mem))
  local daemon_delta=$(( (thttpd + dnscached + syslogd) - (_baseline_thttpd + _baseline_dns + _baseline_syslog) ))
    local step_delta=$((mem - _prev_mem))

    log_metric "mem-check guest mem_used_kb=${mem} delta_kb=${delta} daemon_delta_kb=${daemon_delta} step_kb=${step_delta} host_mem_used_kb=$(host_free_kb)"

    if (( delta > LEAK_TOTAL_KB )); then
        die "memory leak: guest mem_used_kb grew ${delta} KiB above baseline (limit ${LEAK_TOTAL_KB} KiB)"
    fi

    if (( step_delta > LEAK_STEP_KB )); then
        _leak_consec=$((_leak_consec + 1))
        log_metric "mem-warn consecutive growth checks=${_leak_consec}/${LEAK_FAIL_CONSEC}"
    else
        _leak_consec=0
    fi

    if (( _leak_consec >= LEAK_FAIL_CONSEC )); then
        die "memory leak: guest mem_used_kb rose >${LEAK_STEP_KB} KiB for ${LEAK_FAIL_CONSEC} consecutive checks"
    fi

    _prev_mem=$mem
}

main() {
    command -v qemu-system-x86_64 >/dev/null 2>&1 || die "missing qemu-system-x86_64"

    "$ROOT/scripts/mkinitrd.sh"

    local kernel
    kernel="$(resolve_soak_kernel)"

    printf 'qemu-soak: kernel=%s\n' "$kernel" >&2
    printf 'qemu-soak: initrd=%s\n' "$INITRD" >&2
    printf 'qemu-soak: duration=%ss check_interval=%ss\n' "$SOAK_DURATION" "$CHECK_INTERVAL" >&2
    printf 'qemu-soak: log=%s metrics=%s\n' "$LOG" "$METRICS" >&2

    rm -f "$LOG" "$METRICS" "$WATCHDOG"
    : >"$WATCHDOG"

    local qemu_args=(
        -kernel "$kernel"
        -initrd "$INITRD"
        -append "console=ttyS0 rdinit=/init panic=1 init=/init"
        -m "${MEMORY:-256M}"
        -nographic
        -no-reboot
    )
    append_qemu_net_args qemu_args

    if command -v stdbuf >/dev/null 2>&1; then
        qemu-system-x86_64 "${qemu_args[@]}" > >(stdbuf -oL tee "$LOG") 2>&1 &
    else
        qemu-system-x86_64 "${qemu_args[@]}" > >(tee "$LOG") 2>&1 &
    fi
    qemu_pid=$!
    log_metric "qemu pid=${qemu_pid}"

    wait_for_smoke_ok
    wait_for_first_heartbeat
    last_heartbeat_count=$(heartbeat_count)

    local soak_start=$SECONDS
    local soak_end=$((soak_start + SOAK_DURATION))
    local baseline_mem="" baseline_thttpd="" baseline_dns="" baseline_syslog=""
    local prev_mem="" leak_consec=0
    local next_check=$((SECONDS + CHECK_INTERVAL))

    while (( SECONDS < soak_end )); do
        if (( SECONDS >= next_check )); then
            check_hang
            check_memory_leak baseline_mem baseline_thttpd baseline_dns baseline_syslog prev_mem leak_consec
            local elapsed=$((SECONDS - soak_start))
            local remaining=$((soak_end - SECONDS))
            local count
            count=$(heartbeat_count)
            log_metric "ok elapsed=${elapsed}s remaining=${remaining}s heartbeats=${count}"
            next_check=$((SECONDS + CHECK_INTERVAL))
        fi
        sleep 5
    done

    check_hang
    check_memory_leak baseline_mem baseline_thttpd baseline_dns baseline_syslog prev_mem leak_consec

    log_metric "PASS: completed ${SOAK_DURATION}s soak"
    printf 'qemu-soak: passed (%ss)\n' "$SOAK_DURATION" >&2
    kill_qemu_force "$qemu_pid"
    wait "$qemu_pid" 2>/dev/null || true
    qemu_pid=""
    trap - EXIT
}

main "$@"
