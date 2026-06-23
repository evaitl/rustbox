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
# Space-separated target list. Default: all fuzz targets.
TARGETS="${FUZZ_TARGETS:-rash_parse rash_arith rash_run udhcpc thttpd wget dnscached sshd}"

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
    # rustup shims can be silent no-ops in some environments; use the toolchain directly.
    if [[ -d "${HOME}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin" ]]; then
        export PATH="${HOME}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin:${PATH}"
    fi
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
        rash_run) printf '%s' "-timeout=5" ;;
        *) printf '%s' "" ;;
    esac
}

phase1_fuzz() {
    local target=$1
    local corpus="$CORPUS_ROOT/$target"
    local extra
    extra="$(fuzzer_extra_args "$target")"
    log "phase 1: fuzz $target (${DURATION}s) corpus=$corpus"
    (
        cd "$FUZZ"
        export RUSTBOX_APPLETS_CONFIG="$(cd "$(dirname "$APPLET_CONFIG")" && pwd)/$(basename "$APPLET_CONFIG")"
        export CARGO_TARGET_DIR="$TARGET_DIR"
        unset RUSTFLAGS
        # shellcheck disable=SC2086
        cargo fuzz run "$target" "$corpus" -- $extra -max_total_time="$DURATION"
    ) || log "phase 1: $target exited non-zero (crash or timeout; continuing)"
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

    export LLVM_PROFILE_FILE="$prof/${target}-%m.profraw"

    local -a inputs=("$corpus")
    if [[ -d "$artifacts" ]]; then
        while IFS= read -r -d '' f; do
            inputs+=("$f")
        done < <(find "$artifacts" -type f -print0 2>/dev/null)
    fi

    # -runs=0: replay inputs only, no mutation.
    "$bin" "${inputs[@]}" -runs=0 >/dev/null 2>&1 || true

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

    mkdir -p "$CORPUS_ROOT" "$PROF_ROOT" "$TARGET_DIR"

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
}

main "$@"
