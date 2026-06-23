#!/usr/bin/env bash
# Print a bcrypt /etc/passwd line (default user/password: root/rustbox).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
USER="${1:-root}"
UID="${2:-0}"
GID="${3:-0}"
GECOS="${4:-root}"
HOME="${5:-/root}"
SHELL="${6:-/bin/rash}"

hash="$(
    cd "$ROOT"
    cargo test --features applet-sshd --lib -- --ignored --nocapture hash_passwd_line 2>&1 \
        | sed -n "s/^sshd-passwd: //p" \
        | head -1
)"

if [[ -z "$hash" ]]; then
    printf 'gen-sshd-passwd: failed to generate bcrypt hash\n' >&2
    exit 1
fi

printf '%s:%s:%s:%s:%s:%s:%s\n' "$USER" "$hash" "$UID" "$GID" "$GECOS" "$HOME" "$SHELL"
