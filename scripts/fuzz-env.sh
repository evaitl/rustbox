# shellcheck shell=bash
# Source from fuzz scripts: nightly toolchain for cargo-fuzz, no ambient fuzz env leakage.
if [[ -z "${_RUSTBOX_FUZZ_ENV:-}" ]]; then
    _RUSTBOX_FUZZ_ENV=1
    export _RUSTBOX_FUZZ_SAVED_PATH="${PATH}"
fi

unset CARGO_TARGET_DIR RUSTFLAGS
export RUSTBOX_APPLETS_CONFIG="${RUSTBOX_APPLETS_CONFIG:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../fuzz" && pwd)/applets-fuzz.json}"

_nightly="${HOME}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin"
if [[ -d "$_nightly" ]]; then
    export PATH="${_nightly}:${_RUSTBOX_FUZZ_SAVED_PATH}"
fi

fuzz_restore_path() {
    if [[ -n "${_RUSTBOX_FUZZ_SAVED_PATH:-}" ]]; then
        export PATH="$_RUSTBOX_FUZZ_SAVED_PATH"
    fi
    unset RUSTBOX_APPLETS_CONFIG CARGO_TARGET_DIR RUSTFLAGS
}
