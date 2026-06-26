#!/usr/bin/env bash
# Replay libFuzzer artifacts under fuzz/artifacts/<target>/ (must finish within -timeout).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FUZZ="$ROOT/fuzz"
# shellcheck source=scripts/fuzz-env.sh
source "$ROOT/scripts/fuzz-env.sh"

TARGET="${1:-rash_run}"
TIMEOUT="${FUZZ_REGRESSION_TIMEOUT:-5}"
ART_DIR="$FUZZ/artifacts/$TARGET"

log() {
    printf 'fuzz-regression: %s\n' "$*" >&2
}

die() {
    printf 'fuzz-regression: error: %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

main() {
    need_cmd cargo-fuzz
    need_cmd rustc
    [[ -d "$ART_DIR" ]] || die "no artifacts directory: $ART_DIR"

    local -a artifacts=()
    shopt -s nullglob
    artifacts=("$ART_DIR"/*)
    shopt -u nullglob
    [[ ${#artifacts[@]} -gt 0 ]] || die "no artifacts in $ART_DIR"

    local triple bin
    triple="$(rustc -vV | sed -n 's/^host: //p')"
    bin="$FUZZ/target/$triple/release/$TARGET"
    [[ -x "$bin" ]] || {
        log "building $TARGET"
        (cd "$FUZZ" && cargo fuzz build "$TARGET")
    }
    [[ -x "$bin" ]] || die "fuzzer binary missing: $bin"

    log "replaying ${#artifacts[@]} artifact(s) for $TARGET (timeout=${TIMEOUT}s)"
    local extra=()
    [[ "$TARGET" == rash_run ]] && extra=(-timeout="$TIMEOUT")

    local failed=0
    for art in "${artifacts[@]}"; do
        [[ -f "$art" ]] || continue
        log "  $(basename "$art")"
        if "$bin" "${extra[@]}" -runs=1 "$art" >/dev/null 2>&1; then
            log "    ok"
        else
            log "    FAIL"
            failed=$((failed + 1))
        fi
    done

    fuzz_restore_path

    if [[ "$failed" -gt 0 ]]; then
        die "$failed artifact(s) failed"
    fi
    log "all artifacts passed"
}

main "$@"
