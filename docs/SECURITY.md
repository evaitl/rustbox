# Security

RustBox is a BusyBox-style multi-call binary aimed at embedded and initramfs use. Security posture varies by applet; network-facing daemons need extra care.

## Reporting vulnerabilities

If you find a security issue, please report it privately (open a GitHub security advisory or contact the repository maintainer). Do not file public issues for exploitable vulnerabilities until a fix is available.

## `telnetd` (dev-only)

The `telnetd` applet is **experimental and intended for local development only**. It is enabled by default instead of `sshd` to keep the binary smaller. **Do not expose it to untrusted networks.**

### Plaintext warning

**All telnet traffic is unencrypted.** Usernames, passwords, shell input, and shell output travel in cleartext over the network. Anyone on the same network (or along the path) can read or modify sessions. Telnet provides **no confidentiality, integrity, or authentication of the remote peer** beyond a password check at login.

Use telnet only on isolated lab networks, loopback, or serially bridged setups. For any network where eavesdropping is possible, use proper encrypted remote access (OpenSSH, VPN, serial console) instead.

### Known limitations

| Area | Risk |
|------|------|
| **Encryption** | **None.** Passwords and session data are sent in plaintext. |
| **Scope** | Minimal telnet server: login prompt, PTY-backed interactive `rash` shell. No TLS wrapper. |
| **Authentication** | Password-only via bcrypt hashes in `/etc/passwd` (same file as `passwd`). |
| **Credentials** | Initrd template ships default dev account **`root` / `rustbox`**. Change or remove before any real use. |
| **Brute force** | Up to **3 login attempts** per connection; no IP-based rate limiting. |
| **Binding** | Defaults to `0.0.0.0:23` (all interfaces). Restrict with `listen` in `/etc/telnetd.conf` or firewall rules. |
| **Shell access** | Successful login runs interactive `rash` on a PTY with the privileges of the `telnetd` process (typically root in initrd images). |

### Default dev credentials

When using the initrd template (or a passwd file generated from it), the default login is:

| Field | Value |
|-------|-------|
| Username | `root` |
| Password | `rustbox` |

These credentials are for local development and QEMU images only.

### Hardening checklist (if you must run `telnetd`)

1. Bind to loopback (`listen 127.0.0.1`) or block port 23 on untrusted interfaces.
2. Change default passwords with `passwd` after first login.
3. Disable `telnetd` in `applets.json` for production images.
4. Never use telnet across the public Internet or untrusted LANs.

## `sshd` (dev-only, optional)

The `sshd` applet is **experimental and intended for local development only**. It is **disabled by default** in [`applets.json`](../applets.json) and requires the `applet-sshd` Cargo feature. Do not expose it to untrusted networks or use it as a production SSH server.

### Known limitations

| Area | Risk |
|------|------|
| **Scope** | Minimal SSH server; not a full OpenSSH replacement. No SFTP, forwarding, or advanced auth methods. |
| **Authentication** | Password-only. Public-key authentication is rejected. |
| **Credentials** | Usernames and bcrypt hashes live in `/etc/passwd` (password field). There is no `/etc/shadow`; RustBox targets embedded/router use, not multi-user hosts. Service accounts use `x` in field 2. There are no built-in passwords; a missing or empty passwd file prevents startup. The initrd template ships a default dev account: **`root` / `rustbox`** (bcrypt hash in `/etc/passwd`). Change or remove this before any real use. |
| **Brute force** | Failed password attempts are rate-limited to **3 per client IP per minute**. This is a basic throttle, not a substitute for network-level controls or a mature IDS. |
| **Binding** | Defaults to `0.0.0.0:22` (all interfaces). Restrict with `listen` in `/etc/sshd.conf` or firewall rules before exposing the host. |
| **Host keys** | An Ed25519 host key is auto-generated at `/etc/sshd_host_key` if missing. Protect this file; anyone with the key can impersonate the server. |
| **Shell access** | Successful login spawns an interactive `rash` shell with the privileges of the `sshd` process (typically root in initrd images). |
| **Dependencies** | Built on `russh` and `tokio` when the `applet-sshd` feature is enabled. Keep dependencies updated and run `cargo audit` before release. |

### Default dev credentials

When using the initrd template (or a passwd file generated from it), the default login is:

| Field | Value |
|-------|-------|
| Username | `root` |
| Password | `rustbox` |

The password is stored as a bcrypt hash in field 2 of `initrd/template/etc/passwd` (installed as `/etc/passwd`). These credentials are for local development and QEMU images only.

### Hardening checklist (if you must run `sshd`)

1. Set `listen` to loopback or a trusted management network, or block port 22 with a firewall on untrusted interfaces.
2. Use strong, unique passwords and bcrypt hashes in `/etc/passwd`, or run `passwd` after login to rotate credentials.
3. Rotate `/etc/sshd_host_key` if it may have been disclosed.
4. Disable `sshd` in `applets.json` and omit `--features applet-sshd` for production builds unless you accept the risks above. (`sshd` is off by default.)
5. Prefer proper SSH infrastructure (OpenSSH, VPN, serial console) for production remote access.

## `mdev`

- Scans sysfs and listens for kernel uevents (`mdev -s`, `mdev -df`).
- Applies permission rules from `/etc/mdev.conf`; initrd uses this for USB hotplug after devtmpfs mount.
- Does not load kernel modules (`modprobe` not implemented).

## `su`

- Requires effective UID 0. There is no password check; root may switch to any user listed in `/etc/passwd`.
- Clears most environment variables unless `-m`/`-p` is used; sets `HOME`, `USER`, `LOGNAME`, `SHELL`, and `PATH=/bin:/sbin`.
- Use in `inittab` to start daemons as non-root after boot, e.g. `su nobody -c "/bin/thttpd -f"`.
- Do not make the `rustbox` binary setuid; run `su` only from root-owned init scripts.

## `passwd`

- Updates bcrypt hashes in `/etc/passwd` (same file `sshd` reads for authentication). No `/etc/shadow` is used or created.
- Non-root users may change only their own password and must enter the current password.
- Root may change any user listed in `/etc/passwd` without the current password.
- Plaintext passwords are never stored; only bcrypt hashes (`$2a$`, `$2b$`, `$2y$`) are written.

## `dnscached`

- Listens for DNS on UDP; default bind is `0.0.0.0:53` (all interfaces).
- Forwards queries to configured DoH upstreams over TLS.
- When started as root, binds port 53 then drops to the `user` from `/etc/dnscached.conf` (default `dnscache`).
- Set `listen` in `/etc/dnscached.conf` or use firewall rules to restrict access on untrusted networks.
- No per-client rate limiting on UDP queries; abusive clients can generate upstream load.

## `thttpd`

- Fork-per-connection HTTP server with CGI support.
- When started as root, binds the HTTP port then drops to the `user` from `/etc/thttpd.conf` (default `http`).
- Path traversal and `..` segments in URLs are rejected; CGI scripts must be executable and live under the configured document root.
- Request and response sizes are capped, but there is no TLS terminator; do not expose sensitive content over plain HTTP on untrusted networks.

## `wget` / TLS (`wget-tls` feature)

HTTPS support in `wget` is enabled by default (`wget-tls` feature). Disable with `cargo build --no-default-features` or by omitting `wget-tls` from your feature set.

- Optional HTTPS via `rustls` with the Mozilla root store.
- Certificate validation follows `rustls` defaults; pin or proxy if you need stricter policy.

## General

- RustBox runs many applets with elevated privileges in typical initrd setups (PID 1, root). Compromise of any applet may compromise the entire system.
- Static musl binaries reduce dynamic-linker attack surface but do not eliminate memory-safety or logic bugs in `unsafe` syscall paths (network, PTY, fork/exec).
- Review `applets.json` before shipping: disable applets you do not need (especially `telnetd` and optional `sshd`) to shrink attack surface and binary size.

## Dependency audit

Before release builds:

```bash
cargo audit
```

Address reported advisories for enabled features (`applet-dnscached`, `applet-sshd`, `wget-tls`, etc.).
