#!/usr/bin/env bash
# Run the same checks as .github/workflows/ci.yml (plus optional smoke).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

unset RUSTBOX_APPLETS_CONFIG CARGO_TARGET_DIR RUSTFLAGS
_stable="${HOME}/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin"
if [[ -d "$_stable" ]]; then
    export PATH="${_stable}:${PATH}"
fi

run() {
    printf 'ci-local: %s\n' "$*" >&2
    "$@"
}

run cargo fmt --check
run cargo clippy --all-targets
run cargo test

if [[ "${CI_LOCAL_SMOKE:-0}" == 1 ]]; then
    run ./scripts/qemu-smoke.sh
fi

printf 'ci-local: passed\n' >&2
