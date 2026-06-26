#!/usr/bin/env bash
# Build a gzip-compressed cpio initrd containing rustbox as /init.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
STAGING="$ROOT/initrd/staging"
OUTPUT="${INITRD_OUTPUT:-$ROOT/initrd/initrd.img}"
TEMPLATE="$ROOT/initrd/template"
TARGET="${INITRD_TARGET:-}"

log() {
    printf 'mkinitrd: %s\n' "$*" >&2
}

die() {
    printf 'mkinitrd: error: %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

pick_target() {
    if [[ -n "$TARGET" ]]; then
        printf '%s\n' "$TARGET"
        return
    fi
    if rustup target list --installed 2>/dev/null | grep -q '^x86_64-unknown-linux-musl'; then
        printf '%s\n' 'x86_64-unknown-linux-musl'
        return
    fi
    printf '%s\n' ''
}

build_binary() {
    local triple="$1"
    if [[ -n "$triple" ]]; then
        log "building static release binary for $triple"
        rustup target add "$triple" >/dev/null 2>&1 || true
        (
            cd "$ROOT"
            RUSTFLAGS='-C target-feature=+crt-static' \
                cargo build --release --target "$triple"
        )
        printf '%s\n' "$CARGO_TARGET_DIR/$triple/release/rustbox"
        return
    fi

    log "building release binary for host (dynamic linker will be bundled)"
    (cd "$ROOT" && cargo build --release)
    printf '%s\n' "$CARGO_TARGET_DIR/release/rustbox"
}

copy_dynamic_libs() {
    local bin=$1
    local dest=$2

    [[ -f "$bin" ]] || die "binary not found: $bin"
    if ldd "$bin" 2>/dev/null | grep -q 'not a dynamic executable'; then
        return
    fi

    log "bundling dynamic linker and shared libraries"
  while IFS= read -r lib; do
        [[ -n "$lib" && -f "$lib" ]] || continue
        install -D "$lib" "$dest$lib"
    done < <(ldd "$bin" | awk '/=> \// {print $3} /^\// {print $1}')

    for loader in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux.so.2; do
        if [[ -f "$loader" ]]; then
            install -D "$loader" "$dest$loader"
            break
        fi
    done
}

stage_rootfs() {
    local bin=$1

    rm -rf "$STAGING"
    mkdir -p "$STAGING"/{bin,sbin,dev,proc,sys,tmp,run,etc}

    install -m 0755 "$bin" "$STAGING/bin/rustbox"

    "$bin" --list | while read -r applet; do
        [[ -n "$applet" ]] || continue
        ln -sf rustbox "$STAGING/bin/$applet"
    done

    ln -sf bin/rustbox "$STAGING/init"
    local inittab="${INITTAB_TEMPLATE:-$TEMPLATE/etc/inittab}"
    [[ -f "$inittab" ]] || die "inittab template not found: $inittab"
    install -D -m 0644 "$inittab" "$STAGING/etc/inittab"
    if [[ -f "$TEMPLATE/etc/hostname" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/hostname" "$STAGING/etc/hostname"
    fi
    if [[ -f "$TEMPLATE/etc/thttpd.conf" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/thttpd.conf" "$STAGING/etc/thttpd.conf"
    fi
    if [[ -f "$TEMPLATE/etc/dnscached.conf" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/dnscached.conf" "$STAGING/etc/dnscached.conf"
    fi
    if [[ -f "$TEMPLATE/etc/passwd" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/passwd" "$STAGING/etc/passwd"
    fi
    if [[ -f "$TEMPLATE/etc/group" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/group" "$STAGING/etc/group"
    fi
    if [[ -d "$TEMPLATE/var/www" ]]; then
        mkdir -p "$STAGING/var"
        cp -a "$TEMPLATE/var/www" "$STAGING/var/www"
    fi
    if [[ -d "$TEMPLATE/etc/cron.test" ]]; then
        cp -a "$TEMPLATE/etc/cron.test" "$STAGING/etc/"
    fi
    if [[ -f "$TEMPLATE/etc/crontab" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/crontab" "$STAGING/etc/crontab"
    fi
    if [[ -f "$TEMPLATE/etc/logrotate.conf" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/logrotate.conf" "$STAGING/etc/logrotate.conf"
    fi
    if [[ -f "$TEMPLATE/sbin/smoke-test" ]]; then
        install -D -m 0755 "$TEMPLATE/sbin/smoke-test" "$STAGING/sbin/smoke-test"
    fi
    if [[ -f "$TEMPLATE/sbin/run-smoke-test" ]]; then
        install -D -m 0755 "$TEMPLATE/sbin/run-smoke-test" "$STAGING/sbin/run-smoke-test"
    fi
    if [[ -f "$TEMPLATE/sbin/soak-loop" ]]; then
        install -D -m 0755 "$TEMPLATE/sbin/soak-loop" "$STAGING/sbin/soak-loop"
    fi
    if [[ -f "$TEMPLATE/sbin/setup-ping-range" ]]; then
        install -D -m 0755 "$TEMPLATE/sbin/setup-ping-range" "$STAGING/sbin/setup-ping-range"
    fi
    if [[ -f "$TEMPLATE/sbin/fake-ntp-reply" ]]; then
        install -D -m 0755 "$TEMPLATE/sbin/fake-ntp-reply" "$STAGING/sbin/fake-ntp-reply"
    fi
    if [[ -f "$TEMPLATE/etc/mdev.conf" ]]; then
        install -D -m 0644 "$TEMPLATE/etc/mdev.conf" "$STAGING/etc/mdev.conf"
    fi
    if "$bin" --list | grep -qx mdev; then
        mkdir -p "$STAGING/sbin"
        ln -sf ../bin/mdev "$STAGING/sbin/mdev"
    fi

    copy_dynamic_libs "$bin" "$STAGING"
}

pack_initrd() {
    mkdir -p "$(dirname "$OUTPUT")"
    (
        cd "$STAGING"
        find . -print0 | sort -z | cpio --null --create --format=newc --quiet
    ) | gzip -9 >"$OUTPUT"
}

main() {
    need_cmd cargo
    need_cmd cpio
    need_cmd gzip
    need_cmd awk

    local triple shell_bin
    triple="$(pick_target)"
    shell_bin="$(build_binary "$triple")"
    stage_rootfs "$shell_bin"
    pack_initrd

    log "wrote $OUTPUT ($(du -h "$OUTPUT" | awk '{print $1}'))"
    log "run: INITRD=$OUTPUT ./scripts/qemu-test.sh"
}

main "$@"
