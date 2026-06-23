#!/usr/bin/env bash
# Fetch flex/bison/m4 into kernel/.tools when they are not installed system-wide.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="${KERNEL_TOOLS:-$ROOT/kernel/.tools}"

die() {
    printf 'setup-kernel-tools: error: %s\n' "$*" >&2
    exit 1
}

main() {
    command -v apt-get >/dev/null 2>&1 || die "apt-get required to bootstrap kernel tools"

    if command -v flex >/dev/null && command -v bison >/dev/null && command -v m4 >/dev/null; then
        printf 'setup-kernel-tools: flex, bison, and m4 already on PATH\n' >&2
        exit 0
    fi

    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT

    mkdir -p "$TOOLS"
    (
        cd "$tmp"
        apt-get download flex bison m4
        for deb in *.deb; do
            dpkg-deb -x "$deb" "$TOOLS"
        done
    )

    rm -rf "$TOOLS/usr/lib"
    rm -rf "$TOOLS/usr/lib"
    printf 'setup-kernel-tools: installed flex/bison/m4 under %s\n' "$TOOLS" >&2
    printf 'setup-kernel-tools: run ./scripts/build-kernel.sh\n' >&2
}

main "$@"
