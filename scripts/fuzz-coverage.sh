#!/usr/bin/env bash
# Fuzz (phase 1), build with LLVM coverage (phase 2), replay corpus (phase 3).
#
# Phase 4 (report) is left to llvm-cov, for example:
#   llvm-profdata merge -sparse fuzz/profraw/rash_parse/*.profraw -o rash_parse.profdata
#   llvm-cov report fuzz/target/<host-triple>/release/rash_parse -instr-profile=rash_parse.profdata src/applets/sh/parse.rs
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FUZZ="$ROOT/fuzz"
CORPUS_ROOT="$FUZZ/corpus"
PROF_ROOT="$FUZZ/profraw"
# Keep fuzz artifacts under fuzz/ regardless of ambient CARGO_TARGET_DIR.
unset CARGO_TARGET_DIR
TARGET_DIR="$FUZZ/target"
APPLET_CONFIG="$FUZZ/applets-fuzz.json"

# Per-target fuzz time (seconds). Override with FUZZ_DURATION=...
DURATION="${FUZZ_DURATION:-60}"
# Hang check interval during phase 1 (seconds). Override with FUZZ_HANG_CHECK=...
HANG_CHECK="${FUZZ_HANG_CHECK:-300}"
LOG_DIR="${FUZZ_LOG_DIR:-/tmp/rustbox-fuzz-logs}"
# Space-separated target list. Default: all fuzz targets.
TARGETS="${FUZZ_TARGETS:-rash_parse rash_arith rash_run udhcpc thttpd wget dnscached sshd gzip tar}"

log() {
    printf 'fuzz-coverage: %s\n' "$*" >&2
}

die() {
    printf 'fuzz-coverage: error: %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1 (install $2)"
}

setup_toolchain() {
    # shellcheck source=scripts/fuzz-env.sh
    source "$ROOT/scripts/fuzz-env.sh"
    need_cmd cargo "rustup toolchain install nightly"
    need_cmd cargo-fuzz "cargo install cargo-fuzz"
    need_cmd llvm-profdata "llvm (apt install llvm)"
}

ensure_corpus() {
    local target=$1
    local dir="$CORPUS_ROOT/$target"
    if [[ ! -d "$dir" ]] || [[ -z "$(find "$dir" -type f -print -quit 2>/dev/null)" ]]; then
        log "corpus missing for $target; running mk-fuzz-corpus.sh"
        "$ROOT/scripts/mk-fuzz-corpus.sh"
    fi
    [[ -d "$dir" ]] || die "corpus directory not found: $dir"
}

fuzzer_extra_args() {
    local target=$1
    case "$target" in
        rash_run | gzip | tar) printf '%s' "-timeout=5" ;;
        *) printf '%s' "" ;;
    esac
}

proc_state() {
    local pid=$1
    if [[ ! -d "/proc/$pid" ]]; then
        printf 'gone'
        return
    fi
    awk '/^State:/{print $2}' "/proc/$pid/status" 2>/dev/null || printf '?'
}

corpus_progress_marker() {
    local corpus=$1
    if [[ ! -d "$corpus" ]]; then
        printf '0:0'
        return
    fi
    local count newest
    count="$(find "$corpus" -type f 2>/dev/null | wc -l)"
    newest="$(find "$corpus" -type f -printf '%T@\n' 2>/dev/null | sort -n | tail -1)"
    newest="${newest:-0}"
    printf '%s:%s' "$count" "$newest"
}

phase1_fuzz() {
    local target=$1
    local corpus="$CORPUS_ROOT/$target"
    local extra
    extra="$(fuzzer_extra_args "$target")"
    mkdir -p "$LOG_DIR"
    local log="$LOG_DIR/${target}-phase1.log"
    log "phase 1: fuzz $target (${DURATION}s) corpus=$corpus log=$log"
    (
        cd "$FUZZ"
        export RUSTBOX_APPLETS_CONFIG="$(cd "$(dirname "$APPLET_CONFIG")" && pwd)/$(basename "$APPLET_CONFIG")"
        export CARGO_TARGET_DIR="$TARGET_DIR"
        unset RUSTFLAGS
        # shellcheck disable=SC2086
        cargo fuzz run "$target" "$corpus" -- $extra -max_total_time="$DURATION"
    ) >"$log" 2>&1 &
    local pid=$!
    local start=$SECONDS
    local last_marker
    last_marker="$(corpus_progress_marker "$corpus")"
    local last_log_size=0
    local no_progress=0
    local poll=30

    while kill -0 "$pid" 2>/dev/null; do
        sleep "$poll"
        if ! kill -0 "$pid" 2>/dev/null; then
            break
        fi
        local elapsed=$((SECONDS - start))
        local state
        state="$(proc_state "$pid")"
        local marker
        marker="$(corpus_progress_marker "$corpus")"
        local log_size=0
        if [[ -f "$log" ]]; then
            log_size=$(wc -c <"$log")
        fi
        if [[ "$marker" != "$last_marker" ]] || [[ "$log_size" -gt "$last_log_size" ]]; then
            no_progress=0
            last_marker="$marker"
            last_log_size="$log_size"
            log "phase 1: $target alive ${elapsed}s state=$state corpus=$marker log_bytes=$log_size"
        else
            no_progress=$((no_progress + poll))
            if (( no_progress % HANG_CHECK == 0 )) || [[ "$no_progress" -ge "$HANG_CHECK" ]]; then
                log "phase 1: WARN $target no progress for ${no_progress}s (state=$state)"
            fi
            if [[ "$state" == "D" && "$no_progress" -ge "$HANG_CHECK" ]] \
                || [[ "$no_progress" -ge $((HANG_CHECK * 2)) ]]; then
                log "phase 1: killing hung fuzzer $target (pid=$pid)"
                kill -TERM "$pid" 2>/dev/null || true
                sleep 2
                kill -KILL "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
                die "$target appears hung after ${no_progress}s without progress"
            fi
        fi
        if [[ "$elapsed" -ge "$((DURATION + poll))" ]]; then
            log "phase 1: $target exceeded budget (${elapsed}s); sending TERM"
            kill -TERM "$pid" 2>/dev/null || true
            sleep 2
            kill -KILL "$pid" 2>/dev/null || true
            break
        fi
    done

    set +e
    wait "$pid"
    local status=$?
    set -e
    if [[ "$status" -ne 0 ]]; then
        log "phase 1: $target exited $status (crash, timeout, or hang; see $log)"
    else
        log "phase 1: $target finished"
    fi
}

phase2_build() {
    local target=$1
    log "phase 2: coverage build $target"
    (
        cd "$FUZZ"
        export RUSTBOX_APPLETS_CONFIG="$(cd "$(dirname "$APPLET_CONFIG")" && pwd)/$(basename "$APPLET_CONFIG")"
        export CARGO_TARGET_DIR="$TARGET_DIR"
        export RUSTFLAGS="-Cinstrument-coverage"
        cargo fuzz build --release --sanitizer none "$target"
    )
}

fuzzer_bin() {
    local target=$1
    local bin="$TARGET_DIR/release/$target"
    if [[ -x "$bin" ]]; then
        printf '%s' "$bin"
        return
    fi
    local triple
    triple="$(rustc -vV | sed -n 's/^host: //p')"
    bin="$TARGET_DIR/$triple/release/$target"
    [[ -x "$bin" ]] || die "coverage binary missing: $bin (also tried $TARGET_DIR/release/$target)"
    printf '%s' "$bin"
}

phase3_replay() {
    local target=$1
    local bin
    bin="$(fuzzer_bin "$target")"
    local corpus="$CORPUS_ROOT/$target"
    local prof="$PROF_ROOT/$target"
    local artifacts="$FUZZ/artifacts/$target"

    rm -rf "$prof"
    mkdir -p "$prof"
    log "phase 3: replay $target → $prof"

    local -a replay_args=(-runs=0)
    case "$target" in
        gzip | tar | rash_run) replay_args+=(-timeout=5) ;;
    esac

    local idx=0
    while IFS= read -r -d '' f; do
        export LLVM_PROFILE_FILE="$prof/${target}-${idx}.profraw"
        "$bin" "$f" "${replay_args[@]}" >/dev/null 2>&1 || true
        idx=$((idx + 1))
    done < <(
        find "$corpus" -type f -print0 2>/dev/null
        if [[ -d "$artifacts" ]]; then
            find "$artifacts" -type f -print0 2>/dev/null
        fi
    )

    local count
    count="$(find "$prof" -name '*.profraw' -type f | wc -l)"
    if [[ "$count" -eq 0 ]]; then
        die "no profraw files produced for $target (replay failed?)"
    fi
    log "phase 3: $target wrote $count profraw file(s)"
}

merge_profile() {
    local target=$1
    local prof="$PROF_ROOT/$target"
    local out="$FUZZ/${target}.profdata"
    local -a files=()
    shopt -s nullglob
    files=("$prof"/*.profraw)
    shopt -u nullglob
    [[ ${#files[@]} -gt 0 ]] || die "no profraw files to merge for $target"
    llvm-profdata merge -sparse "${files[@]}" -o "$out"
    log "merged profile: $out"
}

main() {
    setup_toolchain
    [[ -f "$APPLET_CONFIG" ]] || die "missing $APPLET_CONFIG"

    mkdir -p "$CORPUS_ROOT" "$PROF_ROOT" "$TARGET_DIR" "$LOG_DIR"

    for target in $TARGETS; do
        log "=== $target ==="
        ensure_corpus "$target"
        phase1_fuzz "$target"
        phase2_build "$target"
        phase3_replay "$target"
        merge_profile "$target"
    done

    log "done. Example report:"
    log "  llvm-cov report $(fuzzer_bin rash_parse) -instr-profile=$FUZZ/rash_parse.profdata $ROOT/src/applets/sh/parse.rs"
    fuzz_restore_path
}

main "$@"
