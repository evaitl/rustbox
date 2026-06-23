# RustBox

A [BusyBox](https://busybox.net/)-style multi-call binary written in Rust. One executable provides many common Unix utilities ("applets"), invoked either as `rustbox <applet>` or via symlinks.

## Build

```bash
cargo build --release
```

The binary is at `target/release/rustbox`.

### Applet configuration

Which applets are compiled in is controlled by [`applets.json`](applets.json). Set an applet to `false` to omit it from the binary:

```json
{
  "applets": {
    "cat": true,
    "sleep": false
  }
}
```

Only listed applets are built; omitted applets default to disabled. Rebuild after editing the file.

### Current applet status

[`applets.json`](applets.json) is the source of truth. As of the default config:

| Category | Applets |
|----------|---------|
| **Enabled** (78 names) | All utilities in the table below, including `sshd` |
| **Disabled** | none in the default config |
| **Aliases** | `sh` → `rash`, `[` → `test` (separate dispatch entries, one implementation each) |

**Cargo features** (see [`Cargo.toml`](Cargo.toml)):

| Feature | Default | Pulls in |
|---------|---------|----------|
| `applet-dig` | yes | `dig` (`simple-dns`) |
| `applet-dnscached` | yes | `dnscached`, TLS/DoH (`rustls`, `simple-dns`) |
| `applet-sshd` | yes | `sshd` (`russh`, `tokio`, `bcrypt`) |
| `wget-tls` | yes | HTTPS in `wget` |

Network daemons (`dnscached`, `thttpd`, `syslogd`, `udhcpc`, `mdev`, `sshd`) and network utilities (`dig`, `logger`, `nc`, `ntpclient`, `ping`, `wget`, …) are Linux-only. Initrd images use [`initrd/template/etc/inittab`](initrd/template/etc/inittab) (`syslogd`, `dnscached`, `thttpd`, `mdev`, `sshd`, `rash`).

Use a different config path with the `RUSTBOX_APPLETS_CONFIG` environment variable:

```bash
RUSTBOX_APPLETS_CONFIG=applets.min.json cargo build --release
```

### Binary size (static musl)

With the default [`applets.json`](applets.json) (all listed applets enabled), a **stripped release** build for **`x86_64-unknown-linux-musl`** is about **5.8 MiB** (**~6,070,000 bytes**; `sshd` and other default features dominate). This is the same static binary [`mkinitrd.sh`](scripts/mkinitrd.sh) uses when the musl target is installed.

Build conditions:

| Setting | Value |
|---------|--------|
| Target | `x86_64-unknown-linux-musl` |
| Profile | `release` (`strip = true`, `lto = true`, `codegen-units = 1`, `panic = "abort"` in [`Cargo.toml`](Cargo.toml)) |
| Linking | Static C runtime: `RUSTFLAGS='-C target-feature=+crt-static'` |
| Applets | Default [`applets.json`](applets.json) (all listed applets enabled) |

Reproduce and measure:

```bash
rustup target add x86_64-unknown-linux-musl
RUSTFLAGS='-C target-feature=+crt-static' \
  cargo build --release --target x86_64-unknown-linux-musl
wc -c target/x86_64-unknown-linux-musl/release/rustbox
```

The result is a single statically linked executable (no `libc.so` or dynamic linker required at runtime). Disabling applets in `applets.json` or using a smaller config reduces the size. Per-applet marginal costs are documented in **[APPLETS.md](docs/APPLETS.md)**; regenerate with [`scripts/measure-applet-sizes.py`](scripts/measure-applet-sizes.py) and [`scripts/patch-applet-sizes-doc.py`](scripts/patch-applet-sizes-doc.py).

### Tests

```bash
cargo test
```

Integration tests live in `tests/` and run the built `rustbox` binary against a temporary workspace.

### Fuzz testing

[`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) targets the `rash` parser, arithmetic evaluator, and builtin-only script runner, plus `thttpd` config/HTTP parsing and `udhcpc` argument parsing. **Requires a nightly Rust toolchain** (`rustup toolchain install nightly`).

```bash
cargo install cargo-fuzz
cargo fuzz list
cargo +nightly fuzz run rash_parse -- -max_total_time=30
cargo +nightly fuzz run rash_arith -- -max_total_time=30
cargo +nightly fuzz run rash_run -- -max_total_time=30 -timeout=5
cargo +nightly fuzz run thttpd -- -max_total_time=30
cargo +nightly fuzz run udhcpc -- -max_total_time=30
cargo +nightly fuzz run wget -- -max_total_time=30
```

`rash_run` clears `PATH` so fuzzing exercises builtins and shell logic without executing host binaries. Corpus seeds live under `fuzz/corpus/<target>/` (gitignored); crashes are written to `fuzz/artifacts/`.

## QEMU

See **[QEMU.md](docs/QEMU.md)** for booting rustbox under QEMU: prerequisites, initrd build, interactive shell (`qemu-shell.sh`), smoke test, networking, and environment variables.

## Raspberry Pi 4

See **[RASPBERRY4.md](docs/RASPBERRY4.md)** for building a flashable Pi 4 SD image (`./scripts/build-pi.sh`).

## Usage

```bash
# Direct invocation
./target/release/rustbox echo hello
./target/release/rustbox ls -la

# List applets
./target/release/rustbox --list

# Symlink style (like BusyBox)
ln -s /path/to/rustbox ~/bin/cat
cat file.txt
```

## Applets

See **[APPLETS.md](docs/APPLETS.md)** for usage, command-line options, and exit status of each applet. See **[VI.md](docs/VI.md)** for the `vi` editor command reference.

| Applet | Description |
|--------|-------------|
| `basename` | Strip directory suffix from paths (`-s` suffix) |
| `cat` | Concatenate and print files |
| `chmod` | Change file modes (`-R`, octal) |
| `chown` | Change file owner/group (`-R`, numeric) |
| `cp` | Copy files and directories (`-r`) |
| `cron` | Run scheduled commands in local time (`-f`, `-c`, `-n`) |
| `cut` | Extract fields from lines (`-d`, `-f`) |
| `date` | Print or format date/time (`-u`, `+FORMAT`) |
| `dd` | Copy and convert data (`if=`, `of=`, `bs=`, `count=`, …) |
| `dig` | DNS lookup via UDP (`@server`, `-t TYPE`, `-x` reverse IPv4) |
| `dirname` | Strip last path component |
| `dnscached` | DNS cache with DoH upstream (`-f`, `-c CONFIG`, `-l ADDR`, `-p PORT`) |
| `dmesg` | Print or clear the kernel ring buffer (`-c`, `-r`) |
| `echo` | Print arguments (`-n` omit newline) |
| `env` | Run command with modified environment (`-i`, `VAR=value`) |
| `false` | Exit with status 1 |
| `find` | Search files (`-name`, `-type`, depth limits) |
| `free` | Print memory usage from `/proc/meminfo` (`-h`) |
| `grep` | Search line patterns (`-i`, `-v`, `-r`, …) |
| `halt` | Halt the system (`-n` skip sync) |
| `head` | Print first lines (`-n`) |
| `hostname` | Print or set the system hostname (`-F`/`-f` file) |
| `ifconfig` | Configure network interfaces (`up`/`down`, address, netmask) |
| `init` | PID 1-style process supervisor (`-f` inittab, `-s` oneshot) |
| `kill` | Send signals to processes (`-l`, `-s`, `-0`) |
| `killall` | Send a signal to processes by name (`-s`) |
| `ln` | Create links (`-s` symbolic) |
| `logger` | Send a message to syslog (`-t TAG`, `-p PRIO`, `-S SOCKET`) |
| `ls` | List directory contents (`-l`, `-a`, `-1`) |
| `mkdir` | Create directories (`-p`) |
| `mknod` | Create device nodes (`-m`, `b`/`c`/`p`) |
| `mount` | Mount filesystems (`-a`, `-t`, `-o`, list mounts) |
| `mdev` | Device manager: `mdev -s` scan, `mdev -df` uevent daemon (USB hotplug) |
| `mv` | Move or rename files |
| `nc` | TCP/UDP netcat (`-l` listen, `-u` UDP, `-w` timeout) |
| `ntpclient` | SNTP time query (`-s` set clock, `-t` timeout) |
| `ping` | Send ICMP echo requests (`-c`, `-W`, `-q`) |
| `pivot_root` | Change root mount (`pivot_root(2)`) |
| `printenv` | Print environment variable values |
| `printf` | Format and print data (`%s`, `%d`, `%x`, …) |
| `ps` | List process IDs and command names |
| `pwd` | Print working directory |
| `rash` / `sh` | Ash-style shell (`-c`, `-i`, scripts, interactive history/editing); `sh` is an alias |
| `readlink` | Print symbolic link target (`-f` canonicalize) |
| `reboot` | Reboot the system (`-n` skip sync) |
| `rm` | Remove files (`-r`, `-f`) |
| `rmdir` | Remove empty directories (`-p`) |
| `route` | Show or modify the IP routing table (`add`, `-n`) |
| `sed` | Stream editor (`s///`, addresses, `-n`) |
| `sleep` | Pause for a number of seconds |
| `sort` | Sort lines (`-r`, `-u`) |
| `stat` | Print file status (`-c`, `-f`, `-L`, `-t`) |
| `switch_root` | Switch initramfs root and exec init |
| `swapoff` | Deactivate swap (`-a` for all) |
| `swapon` | Activate swap on a device or file (`-p` priority) |
| `sync` | Flush filesystem buffers |
| `sysctl` | Read/write kernel parameters (`-a`, `-n`, `key=value`) |
| `syslogd` | Syslog daemon on `/dev/log` (`-f`, `-O`, `-s`) |
| `sshd` | **Dev-only** — SSH server (on by default; see [SECURITY.md](docs/SECURITY.md)) |
| `su` | Drop privileges and run a command or shell as another user (`-c`, `-s`, `-l`) |
| `tail` | Print last lines (`-n`) |
| `top` | Process snapshot sorted by RSS (`-n`, `-d`) |
| `test` / `[` | Evaluate expressions (POSIX tests) |
| `thttpd` | Small HTTP server with CGI (`-f`, `-t`, `-c`, `-p`, `-d`); forks per connection; directory listing via `ls -al` when `index.html` is missing |
| `tr` | Translate or delete characters (`-d`, `-s`) |
| `true` | Exit with status 0 |
| `udhcpc` | DHCP client (`-i`, `-q`, `-n`, `-t`, `-T`) |
| `umount` | Unmount filesystems (`-a`, `-f`, `-l`) |
| `uptime` | Print uptime and load averages |
| `uname` | Print system information |
| `vi` | Small VT100 screen editor (`-T` key script); see [VI.md](docs/VI.md) |
| `wc` | Print line, word, and byte counts |
| `wget` | HTTP/HTTPS GET to stdout or file (`-q`, `-O`) |
| `xargs` | Run commands from stdin input (`-0`, `-n`, `-r`) |

### `dnscached` configuration

Default config path: `/etc/dnscached.conf`. Lines are `key value` ( `#` starts a comment). Defaults: Google DoH at `8.8.8.8` and `8.8.4.4`, `host dns.google`, listen `0.0.0.0:53`.

```text
upstream 8.8.8.8
upstream 8.8.4.4
host dns.google
path /dns-query
listen 0.0.0.0
port 53
user dnscache
```

Each `upstream` line adds an IPv4 resolver address (replacing the built-in list on the first `upstream` directive). Set `host` to the TLS/SNI name for that provider (required when using literal IPs).

When started as root, `thttpd` and `dnscached` bind their listen sockets first, then drop to the configured user (defaults: `http` and `dnscache`). Set `user` to an empty value in the config file to skip privilege dropping. The user must exist in `/etc/passwd`.

### `thttpd` configuration

Default config path: `/etc/thttpd.conf`. Keys use `key=value` or `key value` (`#` starts a comment). Default user after bind: `http`.

```text
port=80
dir=/var/www
cgidir=/var/www/cgi-bin
user=http
```

### `mdev` configuration

Minimal device manager for devtmpfs images and USB hotplug. Config path: `/etc/mdev.conf`. The initrd template runs `mdev -s` after mounting devtmpfs and respawns `mdev -df` to listen for kernel uevents.

```text
# pattern          user:group  mode   [=alias]
sd[a-z]            0:0         660
ttyUSB*            0:0         666
.*                 0:0         660
```

Lines are `pattern uid:gid mode` with optional `=alias` symlink in `/dev`. Patterns support `*`, `?`, and `[a-z]`; the **last** matching rule wins. With devtmpfs, nodes usually exist already; mdev sets ownership/mode and handles devices added after boot (for example USB storage or serial).

### `sshd` configuration (dev-only)

Failed password attempts are rate-limited to **3 per client IP per minute**. See [SECURITY.md](docs/SECURITY.md) for limitations and hardening guidance.

Enabled in default [`applets.json`](applets.json) and [`Cargo.toml`](Cargo.toml) (`applet-sshd` feature). Disable either to omit the applet from the binary.

Credentials come only from `/etc/passwd` (default path for `sshd` and `passwd`). Bcrypt hashes live in the password field (field 2). RustBox is not a multi-user desktop system: there is **no `/etc/shadow`**; login passwords are stored directly in `/etc/passwd`. Service accounts (`http`, `dnscache`, …) use `x` in field 2 and cannot log in via SSH. There are **no built-in usernames or passwords**; if the file is missing, unreadable, or has no valid bcrypt entries, `sshd` refuses to start.

The initrd template ships a dev account in `initrd/template/etc/passwd` (installed as `/etc/passwd`):

- User: `root`
- Password: `rustbox` (stored as a bcrypt hash in the password field, not plaintext)

Config path: `/etc/sshd.conf`. Default listen address is all interfaces (`0.0.0.0`):

```text
listen 0.0.0.0
port 22
passwd /etc/passwd
hostkey /etc/sshd_host_key
```

Passwd format: standard seven colon-separated fields. Only bcrypt hashes (`$2a$`, `$2b$`, `$2y$`) in field 2 are accepted for SSH login; `x` or other placeholders are ignored.

```text
root:$2b$12$...:0:0:root:/root:/bin/rash
```

Generate a new bcrypt hash:

```bash
cargo test --features applet-sshd --lib -- --ignored --nocapture hash_passwd_line
# or: SSHD_HASH_PASS='mypass' cargo test ... hash_passwd_line
```

After login, change the password with the `passwd` applet (updates the same `/etc/passwd` entry):

```text
passwd              # change your own password (prompts for current and new)
passwd -f /etc/passwd root   # root may change any listed user
```

The host key is generated on first run if missing. Successful logins spawn an interactive `rash` shell over a PTY.

## Adding an applet

1. Create `src/applets/myapplet.rs` with `pub fn run(args: &[&str]) -> i32`.
2. Add `#[cfg(applet_myapplet)] pub mod myapplet;` in `src/applets/mod.rs`.
3. Add the applet to `KNOWN_APPLETS` in `build.rs` and an entry in `applets.json`.

Filesystem and process operations go through [`rustix`](https://crates.io/crates/rustix) via `src/sys.rs`.

## License

[Blue Oak Model License 1.0.0](docs/LICENSE.md) — see also [blueoakcouncil.org/license/1.0.0](https://blueoakcouncil.org/license/1.0.0).
