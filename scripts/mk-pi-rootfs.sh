#!/usr/bin/env bash
# Stage an ext4 root filesystem tree for Raspberry Pi 4 (aarch64 musl rustbox).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/pi-musl-env.sh
source "$ROOT/scripts/pi-musl-env.sh"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
STAGING="${PI_ROOTFS_STAGING:-$ROOT/pi4/staging}"
INITRD_TEMPLATE="$ROOT/initrd/template"
PI_TEMPLATE="$ROOT/pi4/template"
TARGET="${PI_TARGET:-aarch64-unknown-linux-musl}"

log() {
    printf 'mk-pi-rootfs: %s\n' "$*" >&2
}

die() {
    printf 'mk-pi-rootfs: error: %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

build_binary() {
    log "building static release binary for $TARGET"
    rustup target add "$TARGET" || die "failed to install $TARGET (run: rustup target add $TARGET)"
    (
        cd "$ROOT"
        RUSTFLAGS='-C target-feature=+crt-static' \
            cargo build --release --target "$TARGET"
    )
    printf '%s\n' "$CARGO_TARGET_DIR/$TARGET/release/rustbox"
}

list_binary() {
    local bin=$1
    if "$bin" --list >/dev/null 2>&1; then
        printf '%s\n' "$bin"
        return
    fi
    local host_bin="$CARGO_TARGET_DIR/release/rustbox"
    if [[ ! -x "$host_bin" ]]; then
        log "building host rustbox for --list (cross target is not runnable here)"
        (cd "$ROOT" && cargo build --release)
    fi
    printf '%s\n' "$host_bin"
}

stage_rootfs() {
    local bin=$1
    local list_bin
    list_bin="$(list_binary "$bin")"

    rm -rf "$STAGING"
    mkdir -p "$STAGING"/{bin,sbin,dev,proc,sys,tmp,run,var,etc}

    install -m 0755 "$bin" "$STAGING/bin/rustbox"
    "$list_bin" --list | while read -r applet; do
        [[ -n "$applet" ]] || continue
        ln -sf rustbox "$STAGING/bin/$applet"
    done
    ln -sf bin/rustbox "$STAGING/init"

    local inittab="${PI_INITTAB:-$PI_TEMPLATE/etc/inittab}"
    [[ -f "$inittab" ]] || die "inittab template not found: $inittab"
    install -D -m 0644 "$inittab" "$STAGING/etc/inittab"

    for rel in \
        etc/hostname \
        etc/thttpd.conf \
        etc/dnscached.conf \
        etc/passwd \
        etc/group \
        etc/mdev.conf \
        etc/sshd.conf \
        etc/telnetd.conf; do
        if [[ -f "$INITRD_TEMPLATE/$rel" ]]; then
            install -D -m 0644 "$INITRD_TEMPLATE/$rel" "$STAGING/$rel"
        fi
    done

    if [[ -d "$INITRD_TEMPLATE/var/www" ]]; then
        cp -a "$INITRD_TEMPLATE/var/www" "$STAGING/var/www"
    fi

    for rel in sbin/setup-ping-range; do
        if [[ -f "$INITRD_TEMPLATE/$rel" ]]; then
            install -D -m 0755 "$INITRD_TEMPLATE/$rel" "$STAGING/$rel"
        fi
    done

    if "$list_bin" --list | grep -qx mdev; then
        ln -sf ../bin/mdev "$STAGING/sbin/mdev"
    fi
}

main() {
    need_cmd cargo
    need_cmd rustup

    setup_pi_musl_env || die "aarch64-linux-musl-gcc not found (install musl cross toolchain or set PI_MUSL_AUTO_FETCH=1)"

    local bin
    bin="$(build_binary)"
    stage_rootfs "$bin"
    log "staged rootfs at $STAGING"
    log "next: ./scripts/mk-pi-image.sh"
}

main "$@"
