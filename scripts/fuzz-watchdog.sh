#!/usr/bin/env bash
# Run a command and kill it if stdout/stderr log file stops growing for CHECK_INTERVAL seconds.
set -euo pipefail

CHECK_INTERVAL="${CHECK_INTERVAL:-300}"
LOG="${1:?usage: fuzz-watchdog.sh LOG command...}"
shift

: >"$LOG.watchdog"

LAST_SIZE=0
STALL_SINCE=""

"$@" >>"$LOG" 2>&1 &
cmd_pid=$!

log_watchdog() {
    printf '%s %s\n' "$(date -Is)" "$*" >>"$LOG.watchdog"
}

while kill -0 "$cmd_pid" 2>/dev/null; do
    sleep "$CHECK_INTERVAL"
    if ! kill -0 "$cmd_pid" 2>/dev/null; then
        break
    fi
    SIZE=$(stat -c%s "$LOG" 2>/dev/null || echo 0)
    if [[ "$SIZE" -eq "$LAST_SIZE" ]]; then
        if [[ -z "$STALL_SINCE" ]]; then
            STALL_SINCE="$(date -Is)"
        fi
        log_watchdog "HANG: log stalled at ${SIZE} bytes since ${STALL_SINCE}; pid=${cmd_pid}"
        log_watchdog "processes: $(pgrep -a -f 'rash_|cargo fuzz|fuzz-coverage' 2>/dev/null | tr '\n' '; ')"
        log_watchdog "log tail:"
        tail -30 "$LOG" >>"$LOG.watchdog" 2>&1 || true
        kill -TERM "$cmd_pid" 2>/dev/null || true
        sleep 5
        kill -KILL "$cmd_pid" 2>/dev/null || true
        wait "$cmd_pid" 2>/dev/null || true
        echo "EXIT:124" >>"$LOG"
        exit 124
    fi
    STALL_SINCE=""
    LAST_SIZE=$SIZE
    log_watchdog "ok: log ${SIZE} bytes"
done

wait "$cmd_pid"
status=$?
echo "EXIT:$status" >>"$LOG"
exit "$status"
