#!/usr/bin/env bash
# Resolve or unpack the QEMU test kernel source tree (linux-7.1).

kernel_source_die() {
    printf 'kernel-source: error: %s\n' "$*" >&2
    exit 1
}

# Print the kernel source tree path. Honors KERNEL_SRC when set.
# Otherwise uses kernel/linux-7.1, extracting kernel/linux-7.1.tar.xz if needed.
resolve_kernel_src() {
    local root=$1

    if [[ -n "${KERNEL_SRC:-}" ]]; then
        printf '%s\n' "$KERNEL_SRC"
        return 0
    fi

    local tree="$root/kernel/linux-7.1"
    if [[ -f "$tree/Makefile" ]]; then
        printf '%s\n' "$tree"
        return 0
    fi

    local tarball="$root/kernel/linux-7.1.tar.xz"
    if [[ -f "$tarball" ]]; then
        printf 'kernel-source: extracting %s\n' "$tarball" >&2
        tar xf "$tarball" -C "$root/kernel"
        if [[ -f "$tree/Makefile" ]]; then
            printf '%s\n' "$tree"
            return 0
        fi
        kernel_source_die "extracted $tarball but $tree/Makefile is missing"
    fi

    kernel_source_die \
        "no kernel source; unpack linux-7.1 under kernel/linux-7.1, place kernel/linux-7.1.tar.xz in kernel/, or set KERNEL_SRC (see docs/QEMU.md)"
}
