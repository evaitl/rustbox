# Applets

RustBox applets are invoked as `rustbox <applet> [arguments...]` or via a symlink named after the applet. Run `rustbox --list` to see which applets are compiled into your binary (controlled by [`applets.json`](../applets.json)).

Unless noted otherwise, operands are file or directory paths. Options must appear before operands. Unknown options print an error to stderr and exit with status 1.

---

### Applet index

`basename`, `cat`, `chmod`, `chown`, `cp`, `cron`, `cut`, `date`, `dd`, `dig`, `dirname`, `dnscached`, `dmesg`, `echo`, `env`, `false`, `find`, `free`, `grep`, `halt`, `head`, `hostname`, `ifconfig`, `init`, `kill`, `killall`, `ln`, `logger`, `ls`, `mkdir`, `mdev`, `mknod`, `mount`, `mv`, `nc`, `ntpclient`, `passwd`, `ping`, `pivot_root`, `printenv`, `printf`, `ps`, `pwd`, `rash`, `readlink`, `reboot`, `rm`, `rmdir`, `route`, `sed`, `sleep`, `sort`, `sshd`, `stat`, `su`, `switch_root`, `swapoff`, `swapon`, `sync`, `sysctl`, `syslogd`, `tail`, `telnetd`, `test`, `thttpd`, `top`, `tr`, `true`, `udhcpc`, `umount`, `uptime`, `uname`, `vi`, `wc`, `wget`, `xargs`.

(`sh` is an alias for `rash`; `[` is an alias for `test`.)

**Build status** (default [`applets.json`](../applets.json)): 78 names enabled (`sshd` disabled). `dig` requires `applet-dig` (`simple-dns`). `dnscached`, `passwd`, `telnetd`, and `wget` HTTPS require the default Cargo features `applet-dig`, `applet-dnscached`, `applet-passwd`, and `wget-tls`. Optional `sshd` requires `applet-sshd`. See [SECURITY.md](SECURITY.md) for `telnetd`, optional `sshd`, `dnscached`, and `thttpd` exposure notes. The `vi` editor is documented in [VI.md](VI.md).

### Binary size notes

Marginal sizes are measured by comparing a stripped release build for `x86_64-unknown-linux-musl` with all default applets enabled (~2,720,448 bytes) against the same build with that applet disabled. Values are rounded to the nearest KiB. Because of 4 KiB page alignment and shared code between applets, marginal costs do not sum exactly to the full binary size. Applets marked ~0 KiB still add dispatch-table entries but little or no unique code. Regenerate with [`scripts/measure-applet-sizes.py`](../scripts/measure-applet-sizes.py) and [`scripts/patch-applet-sizes-doc.py`](../scripts/patch-applet-sizes-doc.py).

---

## `basename`
**Approximate binary size** — ~0 KiB marginal.

Strip directory and optional suffix from pathnames.

**Usage**

```text
basename [-a] [-s SUFFIX] NAME...
basename NAME [SUFFIX]
```

**Options**

| Option | Description |
|--------|-------------|
| `-a` | Accepted for compatibility (no effect) |
| `-s SUFFIX` | Remove a trailing `SUFFIX` from each `NAME` |

With two operands and no `-s`, the second operand is treated as a suffix (legacy form).

**Exit status**

- `0` — success
- `1` — missing operand or invalid option

---

## `cat`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Concatenate files and write them to stdout.

**Usage**

```text
cat [FILE]...
```

With no operands, copies stdin to stdout. With one or more `FILE` operands, prints each file in order.

**Options**

None.

**Exit status**

- `0` — success
- `1` — cannot open a file or write error

---

## `chmod`
**Approximate binary size** — ~0 KiB marginal.

Change file mode bits.

**Usage**

```text
chmod [-R] MODE FILE...
```

**Options**

| Option | Description |
|--------|-------------|
| `-R`, `--recursive` | Change modes recursively through directories |

`MODE` is an octal value (e.g. `755`, `0644`).

**Exit status**

- `0` — success
- `1` — invalid option, bad mode, or chmod error

---

## `chown`
**Approximate binary size** — ~0 KiB marginal.

Change file owner and group.

**Usage**

```text
chown [-R] OWNER[:[GROUP]] FILE...
```

**Options**

| Option | Description |
|--------|-------------|
| `-R`, `--recursive` | Change ownership recursively through directories |

`OWNER` and `GROUP` are numeric IDs. Either part may be omitted (`:group` changes group only).

**Exit status**

- `0` — success
- `1` — invalid option, bad owner/group, or chown error

---

## `cp`
**Approximate binary size** — ~0 KiB marginal.

Copy files and directories.

**Usage**

```text
cp [-r] SOURCE... DEST
```

**Options**

| Option | Description |
|--------|-------------|
| `-r`, `-R`, `--recursive` | Copy directories recursively |

When copying multiple sources, `DEST` must be a directory. For a single directory source without `-r`, the copy fails. Parent directories of the destination are created as needed.

**Exit status**

- `0` — success
- `1` — missing operands, invalid option, or copy error

---

## `cron`
**Approximate binary size** — ~12 KiB marginal.

Run commands on a schedule in local time.

**Usage**

```text
cron [-fb] [-c DIR] [-n]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f` | Run in the foreground (do not daemonize) |
| `-b` | Run in the background (default when `-f` is not set) |
| `-c DIR` | Read crontab files from `DIR` instead of the default locations |
| `-n` | Parse crontabs and exit without running the scheduler |

Without `-c`, reads `/etc/crontab` (six-field lines with a user name) and files under `/var/spool/cron/crontabs` (five-field lines). Schedule times use the local timezone (`TZ` and `/etc/localtime` via libc).

When the scheduler starts (not with `-n`), it logs the effective timezone to stderr.

**Exit status**

- `0` — success
- `1` — invalid option, parse error, or runtime error

---

## `cut`
**Approximate binary size** — ~4 KiB marginal.

Extract selected fields from each line of input.

**Usage**

```text
cut -f LIST [-d DELIM] [FILE]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-f LIST` | Comma-separated field numbers (1-based); required |
| `-d DELIM` | Field delimiter character (default: tab) |

With no `FILE` operands, reads stdin. Each selected field is printed on its own line.

**Exit status**

- `0` — success
- `1` — missing `-f`, invalid option, or I/O error

---

## `date`
**Approximate binary size** — ~4 KiB marginal.

Print or format the current date and time.

**Usage**

```text
date [-u] [+FORMAT]
```

**Options**

| Option | Description |
|--------|-------------|
| `-u` | Use UTC instead of local time |

With no `+FORMAT`, prints a default human-readable timestamp. Format strings follow `strftime(3)` (e.g. `+%Y-%m-%d`).

**Exit status**

- `0` — success
- `1` — invalid option, extra operand, or bad format string

---

## `dd`
**Approximate binary size** — ~4 KiB marginal.

Copy and convert data (BusyBox-style key=value operands).

**Usage**

```text
dd [OPERAND]...
```

**Operands**

| Operand | Description |
|---------|-------------|
| `if=FILE` | Input file (default: stdin) |
| `of=FILE` | Output file (default: stdout) |
| `bs=N` | Block size in bytes (default: 512) |
| `count=N` | Copy at most `N` blocks |
| `skip=N` | Skip `N` input blocks before copying |
| `seek=N` | Seek `N` output blocks before writing |

Operands use the form `name=value`. Unknown operands are rejected.

**Exit status**

- `0` — success
- `1` — invalid operand or I/O error

---

## `dirname`
**Approximate binary size** — ~0 KiB marginal.

Strip the last path component from each operand.

**Usage**

```text
dirname PATH...
```

For a path with no `/`, prints `.`. For `/`, prints `/`.

**Options**

None.

**Exit status**

- `0` — success
- `1` — invalid option

---

## `dig`
DNS lookup via UDP. Requires the `applet-dig` Cargo feature (`simple-dns`). Queries a resolver (default `127.0.0.1:53`; use `dnscached` in the initrd).

**Usage**

```text
dig [@server] [-p port] [-t type] [-x] name
```

**Options**

| Option | Description |
|--------|-------------|
| `@server` | Resolver address (default: `127.0.0.1`) |
| `-p PORT` | Resolver port (default: `53`) |
| `-t TYPE` | Query type (`A`, `AAAA`, `PTR`, `MX`, `TXT`, `NS`, `CNAME`, …) |
| `-x` | Reverse lookup for an IPv4 address (implies `PTR`) |

**Exit status**

- `0` — query answered
- `1` — invalid option, parse error, or DNS failure

---

## `dnscached`
**Approximate binary size** — ~16 KiB marginal.

DNS caching resolver on UDP port 53. Forwards queries to configured DNS-over-HTTPS upstreams. **Enabled** in default [`applets.json`](../applets.json).

**Usage**

```text
dnscached [-f] [-c CONFIG] [-l ADDR] [-p PORT]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f` | Run in foreground (do not daemonize) |
| `-c CONFIG` | Config file (default: `/etc/dnscached.conf`) |
| `-l ADDR` | Override listen address |
| `-p PORT` | Override listen port |
| `-h`, `--help` | Print usage and exit 0 |

**Config file** (`key value` lines; `#` starts a comment). Defaults: Google DoH at `8.8.8.8` and `8.8.4.4`, `host dns.google`, listen `0.0.0.0:53`.

```text
upstream 8.8.8.8
upstream 8.8.4.4
host dns.google
path /dns-query
listen 0.0.0.0
port 53
user dnscache
```

When started as root, binds UDP port 53 first, then drops to the configured user (default `dnscache`). Set `user` to an empty value to skip privilege dropping.

**Exit status**

- `0` — success (daemon runs until killed)
- `1` — invalid option, config error, or bind failure

---

## `dmesg`
**Approximate binary size** — ~0 KiB marginal.

Print the kernel ring buffer (Linux only).

**Usage**

```text
dmesg [-cr]
```

**Options**

| Option | Description |
|--------|-------------|
| `-c`, `--clear` | Clear the ring buffer after printing |
| `-r` | Print the raw buffer (keep syslog priority prefixes) |

**Exit status**

- `0` — success
- `1` — invalid option or kernel log error

---

## `echo`
**Approximate binary size** — ~0 KiB marginal.

Print arguments separated by spaces.

**Usage**

```text
echo [-n] [STRING]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-n` | Do not print a trailing newline (only options before the first string are recognized) |

With no strings, prints a blank line (unless `-n` is set).

**Exit status**

- `0` — always

---

## `env`
**Approximate binary size** — ~4 KiB marginal.

Run a command in a modified environment, or print the environment.

**Usage**

```text
env [-i] [NAME=VALUE]... [COMMAND [ARG]...]
```

**Options**

| Option | Description |
|--------|-------------|
| `-i` | Start with an empty environment |

With no `COMMAND`, prints `NAME=VALUE` lines for the (possibly modified) environment. With `COMMAND`, runs it after applying `NAME=VALUE` assignments.

**Exit status**

- `0` — success (or command exit status when running a command)
- `1` — invalid option or exec failure

---

## `false`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Do nothing and exit with a non-zero status.

**Usage**

```text
false
```

**Options**

None.

**Exit status**

- `1` — always

---

## `find`
**Approximate binary size** — ~4 KiB marginal.

Search for files in a directory hierarchy.

**Usage**

```text
find [PATH...] [EXPRESSION...]
```

**Predicates**

| Predicate | Description |
|-----------|-------------|
| `-name GLOB` | Base name matches shell glob (`*`, `?`) |
| `-type f` | Regular file |
| `-type d` | Directory |
| `-type l` | Symbolic link |
| `-maxdepth N` | Descend at most `N` levels |
| `-mindepth N` | Apply tests only at depth `N` or below |

With no `PATH`, searches `.`. With no predicates, prints all entries. Default action is `-print`.

**Exit status**

- `0` — success
- `1` — errors while searching

---

## `free`
**Approximate binary size** — ~4 KiB marginal.

Print memory usage from `/proc/meminfo`.

**Usage**

```text
free [-h]
```

**Options**

| Option | Description |
|--------|-------------|
| `-h`, `--human` | Human-readable sizes (K, M, G) |

**Exit status**

- `0` — success
- `1` — cannot read `/proc/meminfo`

---

## `grep`
**Approximate binary size** — ~12 KiB marginal.

Search for lines matching a pattern.

**Usage**

```text
grep [-e PATTERN] [-ivnqFwrR] [-Hh] [-c] [-l] [PATTERN] [FILE...]
```

**Options**

| Option | Description |
|--------|-------------|
| `-e PATTERN` | Pattern to match (repeatable) |
| `-i` | Ignore case |
| `-v` | Select non-matching lines |
| `-n` | Prefix lines with line numbers |
| `-q` | Quiet; exit 0 if match found |
| `-F` | Fixed-string match |
| `-w` | Match whole words |
| `-x` | Match whole lines |
| `-c` | Print match counts |
| `-l` | Print filenames with matches only |
| `-H` | Print filename with matches |
| `-h` | Suppress filenames |
| `-r`, `-R` | Recurse into directories |

Without `FILE`, reads stdin. Patterns without meta characters use substring match; otherwise a basic regex subset (`. * + ? ^ $ [ ] | ( )`) is used.

**Exit status**

- `0` — match found
- `1` — no match
- `2` — error

---

## `halt`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Halt the system (Linux only).

**Usage**

```text
halt [-n]
```

**Options**

| Option | Description |
|--------|-------------|
| `-n` | Do not call `sync` before halting |
| `-f` | Accepted for compatibility (no effect) |

Requires sufficient privileges. On non-Linux platforms, prints an error and exits.

**Exit status**

- `0` — halt requested successfully
- `1` — error or unsupported platform

---

## `head`
**Approximate binary size** — ~0 KiB marginal.

Print the first lines of files.

**Usage**

```text
head [-n NUM] [FILE]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-n NUM` | Print the first `NUM` lines (default: 10) |
| `-nNUM` | Same as `-n NUM` |

With no `FILE` operands, reads stdin.

**Exit status**

- `0` — success
- `1` — invalid option, open error, or read error

---

## `hostname`
**Approximate binary size** — ~0 KiB marginal.

Print or set the kernel hostname (`sethostname(2)` / `gethostname(3)`).

**Usage**

```text
hostname [NAME]
hostname -F FILE
hostname -f FILE
```

**Options**

| Option | Description |
|--------|-------------|
| `-F FILE` | Read hostname from `FILE` (first line) and set it |
| `-f FILE` | Same as `-F` |

With no operands, prints the current hostname.

**Exit status**

- `0` — success
- `1` — invalid option, I/O error, name too long, or permission denied

---

## `ifconfig`
**Approximate binary size** — ~8 KiB marginal.

Configure or display network interface addresses and flags.

**Usage**

```text
ifconfig [-a]
ifconfig IFACE
ifconfig IFACE ADDRESS [netmask MASK] [up|down]
```

With no arguments or `-a`, prints all interfaces. With only `IFACE`, prints that interface. Address assignment implies `up` unless `down` is given.

**Exit status**

- `0` — success
- `1` — unknown interface, invalid address, or ioctl error

---

## `init`
**Approximate binary size** — ~12 KiB marginal.

PID 1-style process supervisor driven by an inittab file.

**Usage**

```text
init [-f INITTAB] [-s]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f INITTAB` | Path to inittab (default: `/etc/inittab`) |
| `-s`, `--oneshot` | Run `sysinit`, `wait`, and `once` entries, then exit without respawning |

**Inittab format**

Each non-empty, non-comment line has four colon-separated fields:

```text
id:runlevels:action:command
```

Supported actions:

| Action | Behavior |
|--------|----------|
| `sysinit` | Run once at startup; wait for completion |
| `wait` | Run once; wait for completion |
| `once` | Run once in the background |
| `respawn` | Run in the background; restart when the process exits |

Entries are processed in order: all `sysinit`, then all `wait`, then all `once`, then `respawn` processes are supervised in a loop (unless `-s` is set).

The `command` field is split on whitespace (like `exec` with separate arguments). Use a script path for commands that need spaces or shell syntax (for example `::wait:/sbin/run-smoke-test` instead of `rash -c '…'`).

**Exit status**

- `0` — success (only with `-s`, or if the supervisor loop never returns)
- `1` — invalid option, unreadable inittab, or spawn/wait error

---

## `kill`
**Approximate binary size** — ~4 KiB marginal.

Send signals to processes.

**Usage**

```text
kill [-l] [-s SIG | -SIG] [-0] PID...
```

**Options**

| Option | Description |
|--------|-------------|
| `-l` | List signal names and numbers |
| `-s SIG` | Signal to send (name or number); default `TERM` |
| `-N` | Shorthand for signal `N` (e.g. `-9` for `KILL`) |
| `-0` | Check that each `PID` exists and can be signaled; send no signal |

**Exit status**

- `0` — all signals delivered (or all `-0` checks passed)
- `1` — invalid option, bad signal, or error for one or more `PID`s

---

## `killall`
**Approximate binary size** — ~0 KiB marginal.

Send a signal to processes whose `/proc/PID/comm` name matches.

**Usage**

```text
killall [-s SIG | -SIG] NAME...
```

**Options**

| Option | Description |
|--------|-------------|
| `-s SIG` | Signal to send (name or number); default `TERM` |
| `-N` | Shorthand for signal `N` |

**Exit status**

- `0` — at least one matching process per `NAME` was signaled
- `1` — invalid option, no matching process, or signal error

---

## `ln`
**Approximate binary size** — ~0 KiB marginal.

Create hard or symbolic links.

**Usage**

```text
ln [-s] TARGET LINK...
```

**Options**

| Option | Description |
|--------|-------------|
| `-s`, `--symbolic` | Create symbolic links instead of hard links |

`TARGET` is the existing file; each `LINK` is a new link name to create.

**Exit status**

- `0` — success
- `1` — missing operands, invalid option, or link error

---

## `logger`
Send a syslog message to a Unix domain socket (default `/dev/log`). Pair with `syslogd` in the initrd.

**Usage**

```text
logger [-s] [-t TAG] [-p PRIO] [-S SOCKET] [MESSAGE]
```

**Options**

| Option | Description |
|--------|-------------|
| `-s` | Also print the message to stderr |
| `-t TAG` | Identifier tag (default: `logger`) |
| `-p PRIO` | Syslog priority (`user.notice`, `daemon.err`, numeric 0–23, …) |
| `-S SOCKET` | Unix socket path (default: `/dev/log`) |
| `-h`, `--help` | Print usage and exit 0 |

If `MESSAGE` is omitted, read stdin until EOF.

**Exit status**

- `0` — message sent
- `1` — invalid option or send failure

---

## `ls`
**Approximate binary size** — ~4 KiB marginal.

List directory contents.

**Usage**

```text
ls [-laA1] [FILE]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-l` | Long format (mode, size, mtime, name) |
| `-a`, `-A` | Include names starting with `.` |
| `-1` | One entry per line |

Options may be grouped (e.g. `-la`). With no operands, lists the current directory (`.`). When listing multiple paths, prints a `path:` header before each directory.

**Exit status**

- `0` — all paths listed successfully
- `1` — one or more paths could not be accessed (partial listing may still be printed)

---

## `mkdir`
**Approximate binary size** — ~0 KiB marginal.

Create directories.

**Usage**

```text
mkdir [-p] DIRECTORY...
```

**Options**

| Option | Description |
|--------|-------------|
| `-p`, `--parents` | Create parent directories as needed; do not error if the directory already exists |

**Exit status**

- `0` — success
- `1` — missing operand, invalid option, or create error

---

## `mknod`
**Approximate binary size** — ~0 KiB marginal.

Create a block or character special file, or a FIFO.

**Usage**

```text
mknod [-m MODE] NAME TYPE [MAJOR MINOR]
```

**Operands**

| Operand | Description |
|---------|-------------|
| `NAME` | Path of the node to create |
| `TYPE` | `b` (block), `c` or `u` (character), or `p` (FIFO) |
| `MAJOR` | Device major number (required for `b` and `c`) |
| `MINOR` | Device minor number (required for `b` and `c`) |

**Options**

| Option | Description |
|--------|-------------|
| `-m MODE` | File mode in octal (default: `666`) |

FIFOs (`p`) do not take major/minor numbers.

**Exit status**

- `0` — success
- `1` — missing operand, invalid type/mode, or create error

---

## `mdev`
**Approximate binary size** — ~20 KiB marginal.

Minimal device manager for embedded Linux (Linux only). Scans sysfs, applies `/etc/mdev.conf` rules, and optionally listens for kernel uevents (USB hotplug). **Enabled** in default [`applets.json`](../applets.json). The initrd runs `mdev -s` at boot and respawns `mdev -df`.

**Usage**

```text
mdev [-s] [-d] [-f] [-c CONFIG]
```

With `ACTION` set in the environment (kernel hotplug helper), processes one device without `-s` or `-d`.

**Options**

| Option | Description |
|--------|-------------|
| `-s` | Scan `/sys/devices` and apply rules |
| `-d` | Daemon: listen on netlink for `add`/`remove`/`change` uevents |
| `-f` | Stay in foreground when combined with `-d` |
| `-c CONFIG` | Rules file (default: `/etc/mdev.conf`) |
| `-h`, `--help` | Print usage and exit 0 |

**Config file** — one rule per line: `pattern uid:gid mode` with optional `=alias` symlink under `/dev`. Patterns support `*`, `?`, and `[a-z]`; the **last** matching rule wins.

```text
sd[a-z]       0:0  660
ttyUSB*       0:0  666
.*            0:0  660
```

With **devtmpfs**, device nodes are usually created by the kernel; mdev sets mode/owner and handles late-added devices.

**Exit status**

- `0` — success
- `1` — invalid option or I/O error

---

## `mount`
**Approximate binary size** — ~12 KiB marginal.

Mount filesystems or list mounts.

**Usage**

```text
mount [-a] [-t FSTYPE] [-o OPTS] [SOURCE] TARGET
mount
```

With no arguments, prints the current mount table from `/proc/mounts`.

With a single operand (`TARGET`), mounts using `TARGET` as the mount point (source taken from `/etc/fstab` or similar resolution in the mount helper).

With two operands, mounts `SOURCE` on `TARGET`.

**Options**

| Option | Description |
|--------|-------------|
| `-a` | Mount all entries in `/etc/fstab` (skips `swap` lines) |
| `-t FSTYPE` | Filesystem type |
| `-o OPTS` | Comma-separated mount options (see below) |
| `-r` | Shorthand for `-o ro` |

**`-o` options**

`defaults`, `_netdev`, `ro`, `rw`, `remount`, `bind`, `rbind`, `nosuid`, `nodev`, `noexec`, `noatime`, `nodiratime`, `relatime`, `strictatime`, `dirsync`, `lazytime`, `sync`, `rec`, and `key=value` pairs for filesystem-specific data.

**Exit status**

- `0` — success
- `1` — invalid option, mount failure, or fstab/read error

---

## `mv`
**Approximate binary size** — ~0 KiB marginal.

Move or rename files.

**Usage**

```text
mv SOURCE... DEST
```

When moving multiple sources, `DEST` must be a directory. No options are supported.

**Exit status**

- `0` — success
- `1` — missing operands or move error

---

## `nc`
TCP/UDP netcat-style client and listener (Linux only).

**Usage**

```text
nc [-l] [-u] [-p port] [-w timeout] [HOST] [PORT]
```

**Options**

| Option | Description |
|--------|-------------|
| `-l` | Listen mode (accept one connection or one UDP datagram) |
| `-u` | Use UDP instead of TCP |
| `-p PORT` | Local port (listen mode) or explicit port operand |
| `-w SEC` | Connect/accept timeout in seconds (0 = no limit) |
| `-h`, `--help` | Print usage and exit 0 |

Without `-l`, connects to `HOST:PORT` and copies stdin/stdout to the socket until EOF or hangup.

**Exit status**

- `0` — success
- `1` — invalid option, bind/connect error, or timeout

---

## `ntpclient`
SNTP client (UDP port 123). Prints server time; `-s` sets the system clock via `settimeofday` (requires appropriate privileges).

**Usage**

```text
ntpclient [-s] [-t SEC] [SERVER]
```

**Options**

| Option | Description |
|--------|-------------|
| `-s`, `-S` | Set system clock from the reply |
| `-t SEC`, `-T SEC` | Reply timeout in seconds (default: 5) |
| `-h`, `--help` | Print usage and exit 0 |

Default server is `129.6.15.28` (`time.nist.gov`).

**Exit status**

- `0` — reply received (and clock set if `-s`)
- `1` — invalid option, timeout, or clock set failure

---

## `ping`
**Approximate binary size** — ~4 KiB marginal.

Send ICMP echo requests using Linux ping sockets (`SOCK_DGRAM` + `IPPROTO_ICMP`). Unprivileged use requires a permissive `net.ipv4.ping_group_range` (the QEMU initrd sets `0 2147483647` at boot).

**Usage**

```text
ping [-c COUNT] [-W SEC] [-q] HOST
```

**Options**

| Option | Description |
|--------|-------------|
| `-c COUNT` | Stop after `COUNT` echo requests (default: unlimited) |
| `-W SEC`, `-w SEC` | Per-reply timeout in seconds (default: 3) |
| `-q` | Quiet: only print errors; exit non-zero if no replies |

**Exit status**

- `0` — at least one reply received
- `1` — no replies, permission denied, or invalid arguments

---

## Choosing `pivot_root` vs `switch_root`

Both applets are used during early boot to move from an initramfs to the real root filesystem. They solve the same problem with different mechanisms.

| | `pivot_root` | `switch_root` |
|---|--------------|---------------|
| **Mechanism** | `pivot_root(2)` syscall | `mount --move`, `chroot`, initramfs cleanup |
| **Operands** | `NEW_ROOT PUT_OLD` | `NEW_ROOT INIT [ARG]...` |
| **Execs init** | No — caller runs `exec` afterward | Yes — replaces the process with `INIT` |
| **Old root** | Mounted at `PUT_OLD`; caller must `umount` and `rmdir` it | Deleted in a background child (no umount needed) |
| **Typical use** | Multi-step boot scripts where you control mount/unmount order | One-shot handoff from initramfs to `/sbin/init` |

**Use `pivot_root`** when the current root can be relocated with the `pivot_root(2)` syscall and you want explicit control over what happens next. A common sequence after mounting the real root at `/newroot`:

```text
mkdir /newroot/oldroot
mount --bind /newroot /newroot
pivot_root /newroot /newroot/oldroot
umount -l /oldroot
rmdir /oldroot
exec /sbin/init
```

`PUT_OLD` must be an empty directory inside `NEW_ROOT`, and `NEW_ROOT` must be a mount point (often prepared with `mount --bind`).

**Use `switch_root`** when running from a `tmpfs`/`ramfs` initramfs that cannot be unmounted or pivoted away cleanly. It moves essential virtual filesystems (`/dev`, `/proc`, `/sys`, `/run`) into the new root, swaps the root with `mount --move` + `chroot`, frees initramfs memory by deleting the old tree, and execs `INIT` in one step:

```text
mount /dev/root /newroot
switch_root /newroot /sbin/init
```

On modern kernels the initramfs sits on `rootfs` backed by `nullfs`, so `pivot_root` also works there; `switch_root` remains the simpler all-in-one option when you just need to jump to `/sbin/init`.

---

## `pivot_root`
**Approximate binary size** — ~0 KiB marginal.

Change the root mount with the `pivot_root(2)` syscall.

**Usage**

```text
pivot_root NEW_ROOT PUT_OLD
```

Moves the current root filesystem to `PUT_OLD` and makes `NEW_ROOT` the new root. `PUT_OLD` must be a directory strictly beneath `NEW_ROOT` in the directory hierarchy (typically created empty before the call). `NEW_ROOT` must be a mount point.

This applet only performs the syscall; the caller is responsible for subsequent steps such as `chdir("/")`, unmounting `PUT_OLD`, and execing init. See [Choosing `pivot_root` vs `switch_root`](#choosing-pivot_root-vs-switch_root) above.

**Options**

None.

**Exit status**

- `0` — success
- `1` — bad usage, syscall failure, or unsupported platform

---

## `printenv`
**Approximate binary size** — ~0 KiB marginal.

Print values of environment variables.

**Usage**

```text
printenv [VARIABLE]...
```

With no operands, prints every environment value (one per line, not `NAME=VALUE` form). With operands, prints each variable's value; missing variables are skipped and the exit status is 1 if any were missing.

**Options**

None.

**Exit status**

- `0` — all requested variables found (or listing entire environment)
- `1` — one or more variables not set

---

## `printf`
**Approximate binary size** — ~4 KiB marginal.

Format and print data.

**Usage**

```text
printf FORMAT [ARGUMENT]...
```

Supported conversion specifiers: `%s`, `%c`, `%d`, `%i`, `%u`, `%x`, `%X`, `%o`, and `%%`. No trailing newline is added unless the format string includes one.

**Options**

None.

**Exit status**

- `0` — success
- `1` — missing format, invalid conversion, or bad numeric argument

---

## `ps`
**Approximate binary size** — ~0 KiB marginal.

List running processes (Linux `/proc`).

**Usage**

```text
ps
```

Prints `PID` and command name from `/proc`. No options are supported.

**Exit status**

- `0` — success
- `1` — cannot read `/proc`

---

## `pwd`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Print the current working directory.

**Usage**

```text
pwd
```

Extra arguments are ignored.

**Exit status**

- `0` — success
- `1` — cannot determine current directory

---

## `rash`
**Approximate binary size** — ~164 KiB marginal. `sh` is an alias for the same module.

BusyBox ash-style shell subset for embedded init scripts. Also available as `sh`.

**Usage**

```text
rash [-ci] [SCRIPT]
sh [-ci] [SCRIPT]
```

**Options**

| Option | Description |
|--------|-------------|
| `-c COMMAND` | Execute `COMMAND` as a shell script |
| `-i` | Force interactive mode (read lines from stdin with prompt) |

**Modes**

- `-c COMMAND` — run one command string and exit with its status
- `SCRIPT` — read and execute a script file
- No arguments, stdin is a TTY (or `-i`) — interactive REPL using `PS1` (default `$ `)

**Interactive line editing**

When stdin and stdout are terminals, the REPL provides in-session command history and basic line editing:

- Up/Down — browse previous commands
- Left/Right — move cursor
- Home/End, Ctrl+A/Ctrl+E — start/end of line
- Backspace/Delete — edit text before Enter
- Ctrl+C — clear current line
- Ctrl+D — exit on an empty line

History is kept in memory for the session (not persisted to disk).
- No arguments, stdin is not a TTY — read and execute stdin as a script

**Language features**

- Words, single/double quotes, `#` comments
- Command separators: `;` newline `&&` `||` background `&`
- Pipelines: `cmd1 | cmd2 | …`
- Redirections: `< file` `> file` `>> file` `2> file` `2>&1` `<< DELIM` (here-documents)
- Compound commands: `if … then … [elif … then …]* [else …] fi`, `while … do … done`, `for var in … do … done`, `case WORD in PATTERN) … ;; esac`, `{ … }`, `( … )`
- Functions: `name() { … }` / `function name { … }`, `local`, `return`
- `trap` builtin with INT/HUP/TERM signal delivery at command boundaries
- Variable expansion: `$VAR`, `${VAR}`, `$1`–`$9`, `$#`, `$?`, `$@`, `$(command)`
- Glob expansion (unquoted words)
- Prefix assignments: `VAR=value command …`

**Builtins**

| Builtin | Description |
|---------|-------------|
| `:` / `true` | No-op, exit 0 |
| `false` | Exit 1 |
| `cd [DIR]` | Change directory (`~`, `~/path`, default `$HOME` or `/`) |
| `pwd` | Print working directory |
| `echo [-n] ARGS…` | Print arguments |
| `exit [N]` | Exit shell with status `N` |
| `export [NAME[=VALUE]…]` | Mark variables for export; list with no args |
| `unset NAME…` | Remove variables |
| `set [-e] [-x] [-u] [-o pipefail] [--] ARGS…` | Shell options and positional parameters |
| `shift [N]` | Shift positional parameters |
| `read [VAR]` | Read a line into `VAR` (default `REPLY`) |
| `umask [MODE]` | Print or set umask |
| `exec CMD…` | Replace shell with `CMD` |
| `. FILE` / `source FILE` | Execute script in current shell |
| `eval CMD…` | Parse and execute arguments as shell input |
| `wait [PID]` | Wait for background jobs |
| `test EXPR` / `[ EXPR ]` | POSIX test expressions |
| `trap CMD SIGNAL…` / `trap - SIGNAL…` | Set or clear signal traps (INT, HUP, TERM) |
| `local NAME[=VALUE]…` | Declare function-local variables |
| `return [N]` | Return from a function with status `N` |

**Shell options (`set`)**

| Flag | Description |
|------|-------------|
| `-e` | Exit on first failing command |
| `-x` | Trace commands to stderr |
| `-u` | Treat unset variables as errors during expansion |
| `-o pipefail` | Pipeline status is the rightmost failing command |

**External commands**

Resolved via `PATH` (default `/usr/bin:/bin`) or directly when the name contains `/`.

**Exit status**

- Last command status (0 = success)
- Syntax errors: `2`
- Command not found: `127`

---

## `readlink`
**Approximate binary size** — ~12 KiB marginal.

Print symbolic link values.

**Usage**

```text
readlink [-fn] FILE...
```

**Options**

| Option | Description |
|--------|-------------|
| `-f`, `--canonicalize` | Canonicalize by following symlinks; result must exist |
| `-n`, `--no-suffix` | Do not append a newline (multiple operands separated by spaces) |

Without `-f`, prints the symlink target stored in the link (does not follow further symlinks).

**Exit status**

- `0` — success
- `1` — missing operand, invalid option, or link error

---

## `reboot`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Reboot the system (Linux only).

**Usage**

```text
reboot [-n]
```

**Options**

| Option | Description |
|--------|-------------|
| `-n` | Do not call `sync` before rebooting |
| `-f` | Accepted for compatibility (no effect) |

Requires sufficient privileges. On non-Linux platforms, prints an error and exits.

**Exit status**

- `0` — reboot requested successfully
- `1` — error or unsupported platform

---

## `rm`
**Approximate binary size** — ~4 KiB marginal.

Remove files or directories.

**Usage**

```text
rm [-rf] FILE...
```

**Options**

| Option | Description |
|--------|-------------|
| `-r`, `-R`, `--recursive` | Remove directories and their contents |
| `-f`, `--force` | Ignore missing files |

**Exit status**

- `0` — all removals succeeded
- `1` — one or more removals failed

---

## `rmdir`
**Approximate binary size** — ~0 KiB marginal.

Remove empty directories.

**Usage**

```text
rmdir [-p] DIRECTORY...
```

**Options**

| Option | Description |
|--------|-------------|
| `-p`, `--parents` | Remove `DIR`, then parents, stopping at the first failure |

**Exit status**

- `0` — all directories removed
- `1` — directory not empty, missing, or invalid option

---

## `route`
**Approximate binary size** — ~4 KiB marginal.

Display or modify the IPv4 routing table (netlink on Linux, with `/proc/net/route` fallback for display).

**Usage**

```text
route [-n]
route add [-net|-host] TARGET [gw GATEWAY] [dev IFACE]
route add default gw GATEWAY [dev IFACE]
```

`route del` is not implemented. Legacy `route TARGET gw …` syntax is rejected; use `route add …`.

**Exit status**

- `0` — success
- `1` — invalid syntax, unsupported command, or netlink error

---

## `sed`
**Approximate binary size** — ~8 KiB marginal.

Stream editor for filtering and transforming text.

**Usage**

```text
sed [-n] [-e SCRIPT] SCRIPT [FILE...]
```

**Options**

| Option | Description |
|--------|-------------|
| `-n` | Suppress automatic printing |
| `-e SCRIPT` | Add a script expression |

**Commands**

| Command | Description |
|---------|-------------|
| `s/OLD/NEW/` | Substitute first match per line |
| `s/OLD/NEW/g` | Substitute all matches |
| `d` | Delete line |
| `p` | Print line (useful with `-n`) |
| `q` | Quit |

Optional line addresses: `N`, `N,M`, `$` (last line). Any delimiter may be used instead of `/` in `s` commands. `&` in the replacement inserts the matched text.

**Exit status**

- `0` — success
- `1` — script or I/O error

---

## `sleep`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Pause for a given number of seconds.

**Usage**

```text
sleep SECONDS
```

`SECONDS` may be a floating-point value (e.g. `0.5`).

**Options**

None.

**Exit status**

- `0` — sleep completed
- `1` — missing operand, invalid interval, or sleep error

---

## `sshd`
**Approximate binary size** — ~0 KiB marginal.

Experimental SSH server for **local development only**. Disabled in default [`applets.json`](../applets.json); enable the applet and `applet-sshd` Cargo feature to build it. See [SECURITY.md](SECURITY.md).

**Usage**

```text
sshd [-f] [-c CONFIG] [-l ADDR] [-p PORT] [-P PASSWD]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f` | Run in foreground |
| `-c CONFIG` | Config file (default: `/etc/sshd.conf`) |
| `-l ADDR` | Listen address (default: `0.0.0.0`) |
| `-p PORT` | Listen port (default: `22`) |
| `-P PASSWD` | Passwd file path (default: `/etc/passwd`) |
| `-h`, `--help` | Print usage and exit 0 |

Password-only authentication (bcrypt hashes in passwd file). Public keys are rejected. Failed logins are rate-limited to 3 per client IP per minute.

The initrd template ships a default dev login of **`root` / `rustbox`** (see [SECURITY.md](SECURITY.md)).

**Exit status**

- `0` — success (server runs until killed)
- `1` — missing credentials, invalid option, or startup error

---

## `telnetd`
**Approximate binary size** — ~8 KiB marginal.

Experimental **plaintext** telnet server for **local development only**. Enabled in default [`applets.json`](../applets.json). See [SECURITY.md](SECURITY.md).

**Usage**

```text
telnetd [-f] [-c CONFIG] [-l ADDR] [-p PORT] [-P PASSWD]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f` | Run in foreground |
| `-c CONFIG` | Config file (default: `/etc/telnetd.conf`) |
| `-l ADDR` | Listen address (default: `0.0.0.0`) |
| `-p PORT` | Listen port (default: `23`) |
| `-P PASSWD` | Passwd file path (default: `/etc/passwd`) |
| `-h`, `--help` | Print usage and exit 0 |

Password-only authentication (bcrypt hashes in passwd file). After login, runs interactive `rash -i` on a PTY (fork per connection; no tokio).

The initrd template ships a default dev login of **`root` / `rustbox`** (see [SECURITY.md](SECURITY.md)).

**Exit status**

- `0` — success (server runs until killed)
- `1` — missing credentials, invalid option, or startup error

---

## `passwd`

Change bcrypt password hashes in `/etc/passwd` (default). Enabled with the `applet-passwd` Cargo feature.

**Usage**

```text
passwd [-f FILE] [USER]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f FILE` | Passwd file path (default: `/etc/passwd`) |
| `-h`, `--help` | Print usage and exit 0 |

Non-root users may change only their own password and must enter the current password. Root may change any user listed in the file without the current password. Prompts are hidden on TTYs.

**Exit status**

- `0` — password updated
- `1` — usage error, permission denied, wrong password, or update failure

---

## `su`
**Approximate binary size** — ~8 KiB marginal.

Switch to another user ID and execute a command or login shell. Intended for init scripts and daemons started as root that need to drop privileges.

**Usage**

```text
su [-lmp] [-s SHELL] [-c CMD] [-] USER [ARGS...]
```

**Options**

| Option | Description |
|--------|-------------|
| `-`, `-l` | Login shell (`HOME`, `chdir` to home, `argv[0]` prefixed with `-`) |
| `-m`, `-p` | Preserve environment (default clears most variables) |
| `-s SHELL` | Shell for `-c` or interactive use (default: user's `/etc/passwd` entry, else `/bin/rash`) |
| `-c CMD` | Run `SHELL -c CMD` as the target user |

With `ARGS` after `USER`, executes `ARGS[0]` directly (no shell). Requires effective UID 0; there is no password prompt.

**Examples**

```text
su nobody -c "/bin/thttpd -f"
su daemon /bin/syslogd -f -O /var/log/messages
```

**Exit status**

- `0` — success (or command exit status when executing a command)
- `1` — usage error, unknown user, or privilege drop failure
- `127` — exec failure

---

## `sort`
**Approximate binary size** — ~0 KiB marginal.

Sort lines of text.

**Usage**

```text
sort [-ru] [FILE]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-r` | Reverse sort order |
| `-u` | Suppress duplicate lines (after sorting) |

With no `FILE` operands, reads stdin.

**Exit status**

- `0` — success
- `1` — invalid option or I/O error

---

## `stat`
**Approximate binary size** — ~4 KiB marginal.

Print file status.

**Usage**

```text
stat [-fLtc] FILE...
```

**Options**

| Option | Description |
|--------|-------------|
| `-L` | Follow symlinks |
| `-f` | Do not print errors for missing files |
| `-t` | Terse: print file type only |
| `-c FORMAT` | Print formatted output (`%a` mode, `%n` name, `%s` size, `%F` type, `%u` uid, `%g` gid) |

By default, symlinks are not followed. Without `-c`, prints a multi-line summary.

**Exit status**

- `0` — all files stated successfully
- `1` — one or more files could not be stated

---

## `swapoff`
**Approximate binary size** — ~0 KiB marginal.

Deactivate swap on a device or file.

**Usage**

```text
swapoff [-a] DEVICE...
```

**Options**

| Option | Description |
|--------|-------------|
| `-a` | Disable all swap listed in `/proc/swaps` |

**Exit status**

- `0` — success
- `1` — invalid option or swapoff error

---

## `swapon`
**Approximate binary size** — ~0 KiB marginal.

Activate swap on a device or file.

**Usage**

```text
swapon [-p PRI] DEVICE...
```

**Options**

| Option | Description |
|--------|-------------|
| `-p PRI` | Set swap priority |
| `-d` | Accepted for compatibility (discard hint ignored) |

**Exit status**

- `0` — success
- `1` — invalid option or swapon error

---

## `sync`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Flush filesystem buffers.

**Usage**

```text
sync
```

Calls `sync(2)`. No options.

**Exit status**

- `0` — always

---

## `sysctl`
**Approximate binary size** — ~4 KiB marginal.

Read or write kernel parameters via `/proc/sys`.

**Usage**

```text
sysctl [-a] [-n] [-e] [KEY[=VALUE]]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-a` | Print all keys |
| `-n`, `--values` | Print values only |
| `-N`, `--names` | Print key names only |
| `-e`, `--ignore` | Ignore unknown keys |
| `KEY=VALUE` | Write a parameter |

**Exit status**

- `0` — success
- `1` — invalid option, missing key, or I/O error

---

## `syslogd`
**Approximate binary size** — ~4 KiB marginal.

Simple syslog daemon listening on a Unix datagram socket (default `/dev/log`).

**Usage**

```text
syslogd [-f] [-O LOG] [-s SOCKET]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f`, `-F` | Run in foreground (do not daemonize) |
| `-O LOG` | Log file (default: `/var/log/messages`) |
| `-s SOCKET` | Unix socket path (default: `/dev/log`) |

**Exit status**

- `0` — server exited cleanly (normally runs until killed)
- `1` — bind or I/O error

---

## `switch_root`
**Approximate binary size** — ~8 KiB marginal.

Switch the root filesystem from an initramfs to a mounted real root, then execute `init`. See [Choosing `pivot_root` vs `switch_root`](#choosing-pivot_root-vs-switch_root) for when to prefer this over `pivot_root`.

**Usage**

```text
switch_root NEW_ROOT INIT [ARG]...
```

`NEW_ROOT` must be a directory containing the new root filesystem (typically a mount point). `INIT` is the path to the new init program, usually `/sbin/init`. Remaining arguments are passed to `INIT`.

**Options**

None.

**Behavior**

1. Moves `/dev`, `/proc`, `/sys`, and `/run` into `NEW_ROOT` when possible
2. Changes directory to `NEW_ROOT`, moves it onto `/`, and calls `chroot`
3. Forks a child to delete the old initramfs contents
4. Replaces the current process with `INIT`

**Exit status**

- Does not return on success (`exec` replaces the process)
- `1` — missing operands, pivot failure, unsupported platform, or cannot execute `INIT`

---

## `tail`
**Approximate binary size** — ~4 KiB marginal.

Print the last lines of files.

**Usage**

```text
tail [-n NUM] [FILE]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-n NUM` | Print the last `NUM` lines (default: 10) |
| `-nNUM` | Same as `-n NUM` |

With no `FILE` operands, reads stdin.

**Exit status**

- `0` — success
- `1` — invalid option, open error, or read error

---

## `top`
**Approximate binary size** — ~8 KiB marginal.

Display a periodically refreshed process list sorted by RSS (Linux `/proc`).

**Usage**

```text
top [-n COUNT] [-d SEC]
```

**Options**

| Option | Description |
|--------|-------------|
| `-n COUNT` | Number of updates (`0` means run until interrupted; default: unlimited) |
| `-d SEC` | Delay between updates in seconds (default: 3) |

**Exit status**

- `0` — success
- `1` — invalid option or cannot read `/proc`

---

## `test`
**Approximate binary size** — ~176 KiB marginal. `[` is an alias for the same module; required when `rash`/`sh` is enabled.

Evaluate expression and exit with a status (also available as `[`).

**Usage**

```text
test EXPRESSION
[ EXPRESSION ]
```

When invoked as `[`, a closing `]` operand is accepted and ignored.

**Unary operators**

| Operator | True when |
|----------|-----------|
| `-e PATH` | Path exists |
| `-f PATH` | Regular file |
| `-d PATH` | Directory |
| `-h`, `-L PATH` | Symbolic link |
| `-b`, `-c`, `-p`, `-S` | Block, char, fifo, socket |
| `-r`, `-w`, `-x PATH` | Readable, writable, executable |
| `-s PATH` | Non-zero file size |
| `-n STRING` | String is non-empty |
| `-z STRING` | String is empty |
| `-u`, `-g`, `-k PATH` | setuid, setgid, sticky bit set |

**Binary operators**

| Operator | Meaning |
|----------|---------|
| `=`, `==`, `!=` | String compare |
| `<`, `>` | Lexicographic string compare |
| `-eq`, `-ne`, `-lt`, `-le`, `-gt`, `-ge` | Integer compare |
| `-nt`, `-ot` | File mtime newer/older |
| `-ef` | Same file (device and inode) |

**Logical operators**

| Operator | Meaning |
|----------|---------|
| `!` | Negation |
| `-a` | And |
| `-o` | Or |
| `( ... )` | Grouping |

A lone operand is true if the string is non-empty.

**Exit status**

- `0` — expression is true
- `1` — expression is false
- `2` — syntax error

---

## `thttpd`
**Approximate binary size** — ~36 KiB marginal.

Small HTTP server with CGI/1.1 support. Reads `/etc/thttpd.conf` by default. Accepts connections concurrently by forking a child process per client; CGI scripts are executed in a separate fork from the connection handler. When `index.html` is missing for a directory request, serves the output of `ls -al` on that directory (fork/exec of `/bin/ls`).

**Usage**

```text
thttpd [-f] [-t] [-c CONF] [-p PORT] [-d DIR]
```

**Options**

| Option | Description |
|--------|-------------|
| `-f` | Run in foreground (do not daemonize) |
| `-t` | Built-in smoke test: temporary server on port 18080; checks CGI (`/cgi-bin/smoke-cgi`), directory listing (`/listing-test/`), and `wget` fetches of both |
| `-c CONF` | Config file (default: `/etc/thttpd.conf`) |
| `-p PORT` | Override listen port |
| `-d DIR` | Override document root (CGI dir becomes `DIR/cgi-bin`) |
| `-h`, `--help` | Print usage and exit 0 |

**Config file**

```text
port=80
dir=/var/www
cgidir=/var/www/cgi-bin
user=http
```

Lines starting with `#` are comments. Keys may use `key=value` or `key value`. When started as root, binds the listen port first, then drops to `user` (default `http`). Set `user` to an empty value to skip privilege dropping.

**CGI**

URLs under `/cgi-bin/` (relative to `cgidir` under `dir`) execute scripts that print CGI headers followed by a body. Standard variables such as `REQUEST_METHOD`, `QUERY_STRING`, `CONTENT_LENGTH`, and `PATH_INFO` are set.

**Exit status**

- `0` — server exited cleanly (normally runs until killed)
- `1` — config, bind, or runtime error

---

## `tr`
**Approximate binary size** — ~8 KiB marginal.

Translate or delete characters.

**Usage**

```text
tr [-ds] SET1 [SET2] [FILE]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-d` | Delete characters in `SET1` instead of translating |
| `-s` | Squeeze repeated output characters to a single character |

`SET1` and `SET2` may use ranges (e.g. `a-z`). With no `FILE` operands, reads stdin.

**Exit status**

- `0` — success
- `1` — missing operand, invalid option, or I/O error

---

## `true`
**Approximate binary size** — ~0 KiB marginal. Below page-alignment resolution; folded into shared runtime.

Do nothing and exit successfully.

**Usage**

```text
true
```

**Options**

None.

**Exit status**

- `0` — always

---

## `udhcpc`
**Approximate binary size** — ~4 KiB marginal.

Minimal DHCP client for a single interface.

**Usage**

```text
udhcpc [-i IFACE] [-q] [-n] [-t TRIES] [-T SEC] [IFACE]
```

**Options**

| Option | Description |
|--------|-------------|
| `-i IFACE` | Interface to configure (default: `eth0`) |
| `-t TRIES` | Discovery attempts (default: 3) |
| `-T SEC` | Per-packet timeout in seconds (default: 3) |
| `-q` | Quiet: suppress progress messages |
| `-n` | Exit non-zero if no lease is obtained |
| `-h`, `--help` | Print usage and exit 0 |

A positional `IFACE` overrides `-i`.

**Exit status**

- `0` — lease obtained and applied (`ifconfig` + default route)
- `1` — timeout, no offer, or configuration error

---

## `umount`
**Approximate binary size** — ~0 KiB marginal.

Unmount filesystems.

**Usage**

```text
umount [-afl] [MOUNTPOINT]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-a` | Unmount all mount points from the mount table except `/` (reverse order) |
| `-f` | Force unmount (`MNT_FORCE`) |
| `-l` | Lazy unmount (`MNT_DETACH`) |
| `-r` | Accepted for compatibility (no effect) |

**Exit status**

- `0` — all unmounts succeeded
- `1` — missing operand (without `-a`), invalid option, or unmount error

---

## `uptime`
**Approximate binary size** — ~0 KiB marginal.

Print how long the system has been running and load averages.

**Usage**

```text
uptime
```

Reads `/proc/uptime` and `/proc/loadavg`. No options.

**Exit status**

- `0` — success
- `1` — cannot read `/proc`

---

## `uname`
**Approximate binary size** — ~0 KiB marginal.

Print system information from `/proc/sys/kernel/*`.

**Usage**

```text
uname [-snrma]
```

**Options**

| Option | Description |
|--------|-------------|
| `-a`, `--all` | Print all fields |
| `-s`, `--kernel-name` | Kernel name (`kernel/ostype`) |
| `-n`, `--nodename` | Network hostname (`kernel/hostname`) |
| `-r`, `--kernel-release` | Kernel release (`kernel/osrelease`) |
| `-m`, `--machine` | Machine hardware name (`kernel/arch`) |

With no options, prints the operating system name only (same source as `-s`). Multiple options print fields in the order: sysname, nodename, release, kernel name, machine.

**Exit status**

- `0` — success
- `1` — invalid option or extra operand

---

## `vi`
**Approximate binary size** — ~40 KiB marginal.

Small full-screen text editor for VT100-compatible terminals. This is not a complete vi clone; supported commands, modes, key scripts, and limitations are listed in **[VI.md](VI.md)**.

**Usage**

```text
vi [-T KEYSCRIPT] FILE
```

**Options**

| Option | Description |
|--------|-------------|
| `-T KEYSCRIPT` | Read keys from a script file instead of the terminal (`<Esc>`, `<Enter>`, arrows, …) |
| `-h`, `--help` | Print usage and exit 0 |

Interactive mode requires stdin and stdout to be terminals. The applet uses raw mode and emits ANSI escape sequences for screen redraw.

**Exit status**

- `0` — quit via `:q`, `:wq`, or `:q!`
- `1` — usage error, write failure, or scripted session without `:wq`/`:q!`
- `130` — interrupted (`Ctrl-C`)

---

## `wc`
**Approximate binary size** — ~4 KiB marginal.

Print newline, word, and byte counts.

**Usage**

```text
wc [-lwc] [FILE]...
```

**Options**

| Option | Description |
|--------|-------------|
| `-l` | Print line counts |
| `-w` | Print word counts |
| `-c`, `-m` | Print byte counts |

With no count options, all three are printed. With no `FILE` operands, reads stdin (labeled `-`). When multiple files are given, a `total` line is printed at the end.

**Exit status**

- `0` — all files counted successfully
- `1` — one or more files could not be opened (other files may still be counted)

---

## `wget`
**Approximate binary size** — ~0 KiB marginal.

Download a file over HTTP/1.0 (GET only). IPv4 literal hosts are supported.

**Usage**

```text
wget [-q] [-O FILE|-] URL
```

**Options**

| Option | Description |
|--------|-------------|
| `-q` | Quiet (suppress error messages) |
| `-O FILE` | Write body to `FILE`, or `-` for stdout (default) |
| `-h`, `--help` | Print usage and exit 0 |

**URL form**

`http://HOST[:PORT]/path` — default port 80.

**Exit status**

- `0` — success
- `1` — invalid option, URL, network error, or write failure

---

## `xargs`
**Approximate binary size** — ~4 KiB marginal.

Build and execute command lines from stdin.

**Usage**

```text
xargs [-0] [-r] [-n MAX] [COMMAND [ARG...]]
```

**Options**

| Option | Description |
|--------|-------------|
| `-0` | Input items are null-terminated |
| `-r` | Do not run command if stdin is empty |
| `-n MAX` | Use at most `MAX` arguments per invocation |

Reads whitespace-separated words from stdin (or null-separated with `-0`) and appends them to `COMMAND`. Default command is `echo`.

**Exit status**

- Exit status of the last command run
- `127` — command could not be executed

---
