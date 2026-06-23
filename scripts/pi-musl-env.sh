#!/usr/bin/env bash
# Shared aarch64-linux-musl cross-linker setup for Pi rootfs builds.
# Source from other scripts: source "$(dirname "$0")/pi-musl-env.sh"

_pi_musl_env_root() {
    if [[ -n "${ROOT:-}" ]]; then
        printf '%s\n' "$ROOT"
        return
    fi
    local here
    here="$(cd "$(dirname "${BASH_SOURCE[1]}")/.." && pwd)"
    printf '%s\n' "$here"
}

setup_pi_musl_env() {
    local root toolchain_dir
    root="$(_pi_musl_env_root)"
    toolchain_dir="${PI_MUSL_TOOLCHAIN:-$root/kernel/.tools/aarch64-linux-musl-cross}"

    if command -v aarch64-linux-musl-gcc >/dev/null 2>&1; then
        return 0
    fi

    if [[ -x "$toolchain_dir/bin/aarch64-linux-musl-gcc" ]]; then
        export PATH="$toolchain_dir/bin:$PATH"
        export CC_aarch64_unknown_linux_musl="${CC_aarch64_unknown_linux_musl:-aarch64-linux-musl-gcc}"
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="${CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER:-aarch64-linux-musl-gcc}"
        return 0
    fi

    if [[ "${PI_MUSL_AUTO_FETCH:-1}" == "0" ]]; then
        printf 'pi-musl-env: aarch64-linux-musl-gcc not found (set PI_MUSL_AUTO_FETCH=1 to download)\n' >&2
        return 1
    fi

    command -v curl >/dev/null 2>&1 || {
        printf 'pi-musl-env: need curl or aarch64-linux-musl-gcc on PATH\n' >&2
        return 1
    }
    command -v tar >/dev/null 2>&1 || {
        printf 'pi-musl-env: need tar or aarch64-linux-musl-gcc on PATH\n' >&2
        return 1
    }

    local url="${PI_MUSL_TOOLCHAIN_URL:-https://musl.cc/aarch64-linux-musl-cross.tgz}"
    local archive="${PI_MUSL_TOOLCHAIN_ARCHIVE:-$root/kernel/.tools/aarch64-linux-musl-cross.tgz}"

    printf 'pi-musl-env: fetching %s\n' "$url" >&2
    mkdir -p "$(dirname "$archive")"
    curl -fsSL -o "$archive" "$url"
    mkdir -p "$root/kernel/.tools"
    tar -C "$root/kernel/.tools" -xzf "$archive"

    export PATH="$toolchain_dir/bin:$PATH"
    export CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc
}
