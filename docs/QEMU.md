# QEMU boot test

Boot a minimal initramfs that runs `rustbox` as PID 1 under QEMU. You can use the dedicated kernel from [`kernel/config.qemu`](../kernel/config.qemu) or your host's installed kernel.

## Prerequisites

Install on the host:

| Tool | Used for |
|------|----------|
| Rust toolchain (`cargo`, `rustup`) | Building `rustbox` and the initrd |
| `cpio`, `gzip` | Packing the initrd |
| `qemu-system-x86_64` | Booting the VM |
| Linux kernel build tools (`make`, `flex`, `bison`, `libssl-dev`, `libelf-dev`, `bc`, …) | Building [`kernel/config.qemu`](../kernel/config.qemu) |

On Debian/Ubuntu:

```bash
sudo apt install build-essential flex bison libssl-dev libelf-dev bc cpio gzip qemu-system-x86
```

For a statically linked initrd binary (recommended — no dynamic linker bundled into the image):

```bash
rustup target add x86_64-unknown-linux-musl
```

## Quick start (host kernel)

If `/boot/vmlinuz-$(uname -r)` is present, skip the kernel build and boot with your running kernel:

```bash
./scripts/mkinitrd.sh
./scripts/qemu-test.sh
```

## Interactive shell (`qemu-shell.sh`)

For a serial console with `rash` and **no boot-time smoke test**, use:

```bash
./scripts/qemu-shell.sh
```

This script:

1. Builds `initrd/initrd-shell.img` via `mkinitrd.sh` using [`initrd/template/etc/inittab.shell`](../initrd/template/etc/inittab.shell) instead of the default [`inittab`](../initrd/template/etc/inittab).
2. Boots QEMU through [`scripts/qemu-test.sh`](../scripts/qemu-test.sh) with that initrd.

The shell inittab mounts `proc`/`sys`/`dev`, brings up loopback, sets the hostname from `/etc/hostname`, prints `rustbox shell ready`, and **respawns** `/bin/rash` on the serial console. Unlike the default image, it does **not** run `smoke-test` on boot or start `thttpd`.

Type commands at the `$` prompt. Networking is available (`eth0` via user-mode NAT when `QEMU_NET=1`). Exit QEMU with `Ctrl-A` then `X`.

| Variable | Default | Purpose |
|----------|---------|---------|
| `INITTAB_TEMPLATE` | `initrd/template/etc/inittab.shell` | Inittab file staged as `/etc/inittab` |
| `INITRD_OUTPUT` | `initrd/initrd-shell.img` | Output initrd path |
| `INITRD` | `initrd/initrd-shell.img` (set by the script) | Initrd passed to `qemu-test.sh` |
| `KERNEL` | auto-detect | Same as `qemu-test.sh` |

To build a shell initrd without booting:

```bash
INITTAB_TEMPLATE=initrd/template/etc/inittab.shell \
INITRD_OUTPUT=initrd/initrd-shell.img \
./scripts/mkinitrd.sh
```

Then boot it manually:

```bash
INITRD=initrd/initrd-shell.img ./scripts/qemu-test.sh
```

## QEMU smoke test

End-to-end regression test: builds the initrd, boots QEMU with a virtio NIC (user-mode NAT), runs DHCP via `udhcpc`, and checks the serial log for `smoke: ok`. The test runs [`initrd/template/sbin/smoke-test`](../initrd/template/sbin/smoke-test) during boot (grep, find, sed, cut, tr, sort, shell arithmetic, `break`, `dmesg`, `cron -n`, `date`, `ps`, `kill -0`, `hostname -F /etc/hostname`, `udhcpc`, `ifconfig`, `route`, `ping`, `wget`, `logger`, `syslogd -k` / `/dev/kmsg`, `logrotate -f`, `gzip`, `tar` / `tar -z`, `dig @127.0.0.1`, `ntpclient` with [`fake-ntp-reply`](../initrd/template/sbin/fake-ntp-reply), `nc` loopback, and `thttpd -t` exercising CGI, directory listing via `ls -al`, and `wget` against [`cgi-bin/smoke-cgi`](../initrd/template/var/www/cgi-bin/smoke-cgi) and [`listing-test/`](../initrd/template/var/www/listing-test/)).

**Prerequisites:** `cargo`, `cpio`, `gzip`, `qemu-system-x86_64`, and the rustbox-built kernel at `kernel/vmlinuz` (see below). Kernel build tools (`make`, `flex`, `bison`, `libssl-dev`, `libelf-dev`, `bc`, …) are needed the first time.

Smoke requires a kernel with **virtio-net built in** ([`kernel/config.qemu`](../kernel/config.qemu)). Host `/boot/vmlinuz-*` images often load virtio-net as a module; the initrd has no modules, so **`qemu-smoke.sh` does not use the host kernel**.

Place a Linux 7.1 source tree under `kernel/` (gitignored). Either unpack yourself or drop the tarball in place — [`scripts/build-kernel.sh`](../scripts/build-kernel.sh) and [`scripts/kernel-source.sh`](../scripts/kernel-source.sh) use `kernel/linux-7.1` when present, or extract `kernel/linux-7.1.tar.xz` automatically:

```bash
# Once: fetch the tarball into kernel/ (not tracked in git)
curl -Lo kernel/linux-7.1.tar.xz https://cdn.kernel.org/pub/linux/kernel/v7.x/linux-7.1.tar.xz

# Build kernel/vmlinuz (extracts the tarball on first run if needed)
./scripts/build-kernel.sh

# Run smoke (rebuilds initrd; builds kernel/vmlinuz first if missing)
./scripts/qemu-smoke.sh
```

`qemu-smoke.sh` uses `KERNEL` when set, otherwise `kernel/vmlinuz`, otherwise runs `build-kernel.sh`. It exits 0 when the serial log contains `smoke: ok`. On success it **kills QEMU immediately** instead of waiting for the guest to shut down. The run is limited to **`SMOKE_TIMEOUT` seconds (default 60)**; if the guest does not finish in time, the script sends SIGKILL to QEMU (and related child processes) and fails. Inside the guest, [`sbin/smoke-test`](../initrd/template/sbin/smoke-test) enforces the same wall-clock limit and prints `smoke: timeout` on expiry; init halts the VM if the smoke test fails so the host does not sit idle until the outer timeout. On failure, inspect `initrd/qemu-smoke.log`.

Override the kernel image explicitly:

```bash
KERNEL=/path/to/vmlinuz ./scripts/qemu-smoke.sh
```

| Variable | Default | Purpose |
|----------|---------|---------|
| `KERNEL_SRC` | `kernel/linux-7.1` (or extract `kernel/linux-7.1.tar.xz`) | Unpacked Linux source for `build-kernel.sh` |
| `KERNEL` | `kernel/vmlinuz` (built on demand) | Kernel image for QEMU (**smoke only**; no host fallback) |
| `SMOKE_TIMEOUT` | `60` | Host wall-clock limit if `smoke: ok` never appears; guest `smoke-test` uses the same default |
| `SMOKE_LOG` | `initrd/qemu-smoke.log` | QEMU serial output log |

## QEMU soak test

Long-running stability test: boots the same initrd as smoke, waits for `smoke: ok`, then keeps QEMU running while [`sbin/soak-loop`](../initrd/template/sbin/soak-loop) exercises `wget`, `dig`, and `ping` every 30 seconds. Each guest heartbeat prints guest `mem_used_kb` (from `/proc/meminfo`) and per-daemon RSS (`thttpd`, `dnscached`, `syslogd` via `/proc/*/cmdline` + `VmRSS`).

The host script [`scripts/qemu-soak.sh`](../scripts/qemu-soak.sh) checks every **15 minutes** (default) for:

- **Hangs** — no new `soak: heartbeat` lines since the previous check; QEMU process still alive
- **Memory leaks** — guest `mem_used_kb` growth vs. baseline (`LEAK_TOTAL_KB`, default 32 MiB) or sustained step growth (`LEAK_STEP_KB` × `LEAK_FAIL_CONSEC`)

```bash
# Default: 12 hours, 15-minute checks
./scripts/qemu-soak.sh

# Shorter local run
SOAK_DURATION=3600 CHECK_INTERVAL=300 ./scripts/qemu-soak.sh
```

| Variable | Default | Purpose |
|----------|---------|---------|
| `SOAK_DURATION` | `43200` (12h) | Total soak time after `smoke: ok` |
| `CHECK_INTERVAL` | `900` (15m) | Hang/leak check period |
| `BOOT_TIMEOUT` | `600` | Wait for `smoke: ok` |
| `LEAK_TOTAL_KB` | `32768` | Fail if guest used memory grows this much above baseline |
| `LEAK_STEP_KB` | `2048` | Per-check growth counted toward consecutive leak failures |
| `LEAK_FAIL_CONSEC` | `3` | Fail after this many consecutive over-threshold steps |
| `SOAK_LOG` | `initrd/qemu-soak.log` | QEMU serial output |
| `SOAK_METRICS` | `initrd/qemu-soak.metrics` | Host check log (timestamps, memory deltas) |

Run in the background for overnight soaks:

```bash
nohup ./scripts/qemu-soak.sh > /tmp/qemu-soak-host.log 2>&1 &
```

## Full workflow (dedicated kernel + initrd)

From the repository root:

**1. Fetch a Linux 7.1 source tree** (once; gitignored under `kernel/`):

```bash
curl -Lo kernel/linux-7.1.tar.xz https://cdn.kernel.org/pub/linux/kernel/v7.x/linux-7.1.tar.xz
```

**2. Build the kernel** using [`kernel/config.qemu`](../kernel/config.qemu):

```bash
./scripts/build-kernel.sh
```

This uses `kernel/linux-7.1` or extracts `kernel/linux-7.1.tar.xz`, copies the defconfig, runs `make olddefconfig`, builds `arch/x86/boot/bzImage`, and installs `kernel/vmlinuz`. Override paths if needed:

```bash
KERNEL_SRC=/path/to/linux \
KERNEL_CONFIG=$PWD/kernel/config.qemu \
KERNEL_INSTALL=$PWD/kernel/vmlinuz \
JOBS=8 \
./scripts/build-kernel.sh
```

**3. Build the initrd**:

```bash
./scripts/mkinitrd.sh
```

`mkinitrd.sh` builds a release `rustbox`, stages `/init` and applet symlinks under `/bin`, installs an inittab (default [`initrd/template/etc/inittab`](../initrd/template/etc/inittab); override with `INITTAB_TEMPLATE`), [`etc/hostname`](../initrd/template/etc/hostname), [`etc/mdev.conf`](../initrd/template/etc/mdev.conf), [`etc/thttpd.conf`](../initrd/template/etc/thttpd.conf), [`var/www`](../initrd/template/var/www), and [`sbin`](../initrd/template/sbin) helpers (`smoke-test`, `fake-ntp-reply`, `run-smoke-test`, `setup-ping-range`; `/sbin/mdev` symlink when the applet is enabled), then writes `initrd/initrd.img` (gzip-compressed cpio). Override the output path:

```bash
INITRD_OUTPUT=$PWD/initrd/initrd.img ./scripts/mkinitrd.sh
```

**4. Boot in QEMU**:

```bash
KERNEL=$PWD/kernel/vmlinuz INITRD=$PWD/initrd/initrd.img ./scripts/qemu-test.sh
```

If `KERNEL` is unset, `qemu-test.sh` looks for `kernel/vmlinuz`, then falls back to the host kernel. The default kernel command line is `console=ttyS0 rdinit=/init panic=1 init=/init`. By default QEMU also attaches a virtio NIC with user-mode networking (`-netdev user,id=net0 -device virtio-net-pci,netdev=net0`); set `QEMU_NET=0` to disable.

## Inside the VM

### Default initrd (`initrd.img`)

Boot sequence from [`initrd/template/etc/inittab`](../initrd/template/etc/inittab):

1. **sysinit** — mount `proc`, `sysfs`, `devtmpfs`, `devpts`; `mdev -s`; create `/var/log`; run `setup-ping-range`; bring up `lo`; set hostname from `/etc/hostname`; start `syslogd -k` (userspace syslog plus `/dev/kmsg` — no separate `klogd`), `cron`, `dnscached`, and `thttpd`.
2. **wait** — run [`sbin/smoke-test`](../initrd/template/sbin/smoke-test) via `run-smoke-test` (halts the VM on failure).
3. **respawn** — `mdev -df` (USB hotplug), `telnetd -f`, and an interactive `rash` shell on the serial console.

### Shell initrd (`initrd-shell.img`)

Boot sequence from [`initrd/template/etc/inittab.shell`](../initrd/template/etc/inittab.shell) (used by `qemu-shell.sh`):

1. **sysinit** — same mounts and network setup as above.
2. **respawn** — `rash` only (no smoke test, no `thttpd`).

### Examples

```text
ls /
uname -a
mount
ls /sys/class/net
cat /var/www/index.html
```

The guest has loopback plus a virtio Ethernet device (typically `eth0`) when networking is enabled. User-mode NAT provides outbound connectivity from the guest; use `QEMU_NETDEV=user,id=net0,hostfwd=tcp::5555-:80` to forward host ports. `thttpd` reads `/etc/thttpd.conf`, serves static files from `/var/www`, runs CGI from `/var/www/cgi-bin/`, and handles each client in a forked child.

Exit QEMU with `Ctrl-A` then `X`.

## Environment variables

| Variable | Script | Default | Purpose |
|----------|--------|---------|---------|
| `KERNEL_SRC` | `build-kernel.sh` | `kernel/linux-7.1` (or extract `kernel/linux-7.1.tar.xz`) | Path to Linux source tree |
| `KERNEL_CONFIG` | `build-kernel.sh` | `kernel/config.qemu` | Kernel defconfig to install |
| `KERNEL_INSTALL` | `build-kernel.sh` | `kernel/vmlinuz` | Where to install `bzImage` |
| `JOBS` | `build-kernel.sh` | `nproc` | Parallel `make` jobs |
| `INITRD_OUTPUT` | `mkinitrd.sh` | `initrd/initrd.img` | Output initrd path |
| `INITTAB_TEMPLATE` | `mkinitrd.sh` | `initrd/template/etc/inittab` | Inittab file staged as `/etc/inittab` |
| `INITRD_TARGET` | `mkinitrd.sh` | `x86_64-unknown-linux-musl` if installed | Rust cross-compile target |
| `KERNEL` | `qemu-test.sh`, `qemu-smoke.sh` | `kernel/vmlinuz` (`qemu-test.sh` may fall back to host kernel; smoke does not) | Kernel image (`bzImage` / `vmlinuz`) |
| `INITRD` | `qemu-test.sh`, `qemu-shell.sh` | `initrd/initrd.img` (or `initrd-shell.img` from `qemu-shell.sh`) | Initrd image |
| `SMOKE_TIMEOUT` | `qemu-smoke.sh` | `30` | Host wall-clock limit if `smoke: ok` never appears; guest `smoke-test` uses the same default |
| `SMOKE_LOG` | `qemu-smoke.sh` | `initrd/qemu-smoke.log` | QEMU serial log from smoke test |
| `APPEND` | `qemu-test.sh` | `console=ttyS0 rdinit=/init panic=1 init=/init` | Kernel command line |
| `MEMORY` | `qemu-test.sh` | `256M` | Guest RAM |
| `QEMU_NET` | `qemu-test.sh`, `qemu-smoke.sh` | `1` | Set to `0` to omit `-netdev`/NIC |
| `QEMU_NETDEV` | `qemu-test.sh`, `qemu-smoke.sh` | `user,id=net0` | QEMU `-netdev` backend |
| `QEMU_NET_DEVICE` | `qemu-test.sh`, `qemu-smoke.sh` | `virtio-net-pci,netdev=net0` | QEMU NIC device |

## Rebuild after changes

- **Applet or rustbox code changes** — rerun `./scripts/mkinitrd.sh` (kernel rebuild not needed).
- **Inittab or initrd layout** — edit `initrd/template/`, then rerun `./scripts/mkinitrd.sh` (or `./scripts/qemu-shell.sh` for the shell image).
- **Kernel features** — edit `kernel/config.qemu`, then rerun `./scripts/build-kernel.sh`.
