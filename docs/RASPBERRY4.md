# Raspberry Pi 4 image

Build a flashable **SD card image** for **Raspberry Pi 4** (64-bit) with:

- **FAT32 `/boot`** — Pi GPU firmware, kernel, device tree
- **ext4 root** — static `aarch64-unknown-linux-musl` RustBox as PID 1 (`/init`)

The image is a wired appliance (Ethernet, `udhcpc`, `thttpd`, `dnscached`, `rash` shell). WiFi is not included.

## Prerequisites

Install on a **Linux build host** (Debian/Ubuntu examples):

```bash
sudo apt update
sudo apt install --no-install-recommends \
  build-essential flex bison libssl-dev libelf-dev bc python3 rsync kmod cpio \
  gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu \
  e2fsprogs dosfstools parted git curl

rustup target add aarch64-unknown-linux-musl
```

| Item | Purpose |
|------|---------|
| Rust + `aarch64-unknown-linux-musl` | Static RustBox binary |
| `gcc-aarch64-linux-gnu` | Cross-compile Pi kernel |
| `e2fsprogs`, `dosfstools`, `parted` | ext4 + FAT SD image |
| Raspberry Pi Linux source | Pi 4 kernel + DTB (download below) |

### Kernel source

Clone the Raspberry Pi kernel tree (or use `kernel/linux-rpi` in this repo):

```bash
git clone --depth=1 https://github.com/raspberrypi/linux -b rpi-6.6.y kernel/linux-rpi
```

### Board

| Item | Notes |
|------|--------|
| Raspberry Pi 4 | 2 GB+ RAM recommended |
| MicroSD card | 8 GB+ |
| USB-C power supply | 3 A recommended |
| USB keyboard + HDMI monitor | Console on `tty1` (`rash` shell) |
| Ethernet cable | First boot uses `eth0` + `udhcpc` |
| USB SD reader | Flash image from host |

Connect a **USB keyboard** and **HDMI display** before power-on. The login shell runs on the HDMI virtual terminal (`tty1`).

---

## Quick build

From the repository root:

```bash
./scripts/build-pi.sh
```

Or step by step:

```bash
./scripts/fetch-pi-firmware.sh   # start4.elf, fixup4.dat
./scripts/build-pi-kernel.sh     # Image + DTB → kernel/pi/
./scripts/mk-pi-rootfs.sh          # rootfs → pi4/staging/
sudo ./scripts/mk-pi-image.sh    # pi4/sdcard.img
```

Outputs:

| Path | Contents |
|------|----------|
| `kernel/pi-firmware/` | GPU boot files (`start4.elf`, `fixup4.dat`) |
| `kernel/pi/Image` | arm64 kernel (copied as `kernel8.img` on `/boot`) |
| `kernel/pi/bcm2711-rpi-4-b.dtb` | Pi 4 device tree |
| `pi4/staging/` | Rootfs directory tree (intermediate) |
| `pi4/sdcard.img` | Flashable SD image |

### Scripts

| Script | Purpose |
|--------|---------|
| [`scripts/build-pi.sh`](../scripts/build-pi.sh) | One-shot image build |
| [`scripts/fetch-pi-firmware.sh`](../scripts/fetch-pi-firmware.sh) | Download Pi 4 GPU firmware |
| [`scripts/build-pi-kernel.sh`](../scripts/build-pi-kernel.sh) | Build `Image` + DTB |
| [`scripts/mk-pi-rootfs.sh`](../scripts/mk-pi-rootfs.sh) | Stage ext4 root tree |
| [`scripts/mk-pi-image.sh`](../scripts/mk-pi-image.sh) | Assemble SD image (needs root) |
| [`scripts/refresh-pi-image-rootfs.sh`](../scripts/refresh-pi-image-rootfs.sh) | Rewrite root partition only (no sudo) |

---

## Step-by-step

### 0. Fetch Pi GPU firmware

Pi 4 boards need `start4.elf` and `fixup4.dat` on the FAT boot partition:

```bash
./scripts/fetch-pi-firmware.sh
```

(`mk-pi-image.sh` and `build-pi.sh` run this automatically if missing.)

### 1. Build the kernel

```bash
# defaults to kernel/linux-rpi when present
./scripts/build-pi-kernel.sh
```

This runs `bcm2711_defconfig`, merges [`kernel/config.pi4`](../kernel/config.pi4), and installs:

- `kernel/pi/Image`
- `kernel/pi/bcm2711-rpi-4-b.dtb`

Override cross compiler if needed:

```bash
CROSS_COMPILE=aarch64-linux-gnu- ./scripts/build-pi-kernel.sh
```

### 2. Build the root filesystem staging tree

```bash
./scripts/mk-pi-rootfs.sh
```

Cross-compiles RustBox for `aarch64-unknown-linux-musl` and populates `pi4/staging/`:

- `/init` → `rustbox` (PID 1)
- `/bin/*` applet symlinks
- `/etc/inittab` from [`pi4/template/etc/inittab`](../pi4/template/etc/inittab)
- Config and web content from [`initrd/template/`](../initrd/template/) (`passwd`, `thttpd.conf`, `dnscached.conf`, `/var/www`, …)

Boot sequence (see inittab): mount `proc`/`sys`, `mdev`, loopback, `udhcpc` on onboard `eth0`, then background `dnscached`, `thttpd`, `telnetd`, and `rash -i` on `tty1` (HDMI + USB keyboard).

`mk-pi-rootfs.sh` downloads an aarch64 musl cross linker from [musl.cc](https://musl.cc) when `aarch64-linux-musl-gcc` is not on `PATH` (disable with `PI_MUSL_AUTO_FETCH=0`).

### 3. Assemble the SD image

**Run as root** (loop mounts + mkfs):

```bash
sudo ./scripts/mk-pi-image.sh
```

Creates `pi4/sdcard.img` (default **2 GiB**, **128 MiB** FAT boot + ext4 root). The boot partition includes GPU firmware, kernel, DTB, `config.txt`, and `cmdline.txt`.

| Variable | Default | Purpose |
|----------|---------|---------|
| `PI_IMAGE` | `pi4/sdcard.img` | Output image path |
| `PI_IMAGE_SIZE` | `2G` | Total image size |
| `PI_BOOT_SIZE_MB` | `128` | FAT boot partition size |
| `PI_ROOTFS_STAGING` | `pi4/staging` | Staged rootfs input |
| `PI_KERNEL_DIR` | `kernel/pi` | Kernel + DTB input |

**Important:** run `./scripts/mk-pi-rootfs.sh` as your normal user **before** `sudo ./scripts/mk-pi-image.sh` so Cargo does not run as root.

To update only the root partition in an existing image (no sudo):

```bash
./scripts/mk-pi-rootfs.sh
./scripts/refresh-pi-image-rootfs.sh
```

---

## Flash and boot

### Flash the SD card

**Find the device carefully** (`lsblk`). The target is the whole card (e.g. `/dev/sdb`), not a partition.

```bash
sudo dd if=pi4/sdcard.img of=/dev/sdX bs=4M conv=fsync status=progress
sync
```

Or use [Raspberry Pi Imager](https://www.raspberrypi.com/software/) → “Use custom” and select `pi4/sdcard.img`.

### Boot

1. Insert SD card into the Pi 4.
2. Connect an **HDMI monitor** and **USB keyboard**.
3. Connect **Ethernet** to a network with DHCP.
4. Connect **USB-C power**.

You should see `rustbox pi4 ready` on the HDMI display, then a `rash` prompt. Only `rash` is attached to `tty1`; `thttpd`, `telnetd`, and `dnscached` run in the background.

### Access

| Method | How |
|--------|-----|
| HDMI + USB keyboard | `rash` shell on `tty1` (root, no password) |
| HTTP | `http://<pi-ip>/` (`thttpd`, port 80) |
| Telnet | `telnetd` on port 23 — **plaintext** (default dev account `root`/`rustbox`; see [SECURITY.md](SECURITY.md)) |

Get the Pi’s IP from your router DHCP list, or on the HDMI shell run `ifconfig eth0`.

---

## Customization

| File | Purpose |
|------|---------|
| [`pi4/template/etc/inittab`](../pi4/template/etc/inittab) | Boot services and shell |
| [`pi4/template/boot/config.txt`](../pi4/template/boot/config.txt) | Pi firmware (`kernel`, `device_tree`, HDMI) |
| [`pi4/template/boot/cmdline.txt`](../pi4/template/boot/cmdline.txt) | Kernel cmdline (`console=tty1`, `root=/dev/mmcblk0p2`, `init=/init`) |
| [`initrd/template/etc/thttpd.conf`](../initrd/template/etc/thttpd.conf) | HTTP server (user `http`) |
| [`initrd/template/etc/dnscached.conf`](../initrd/template/etc/dnscached.conf) | DNS cache (user `dnscache`) |
| [`initrd/template/etc/passwd`](../initrd/template/etc/passwd) | Users for privilege dropping |

After edits, rebuild:

```bash
./scripts/mk-pi-rootfs.sh
sudo ./scripts/mk-pi-image.sh
```

---

## Troubleshooting

| Problem | Things to check |
|---------|------------------|
| `missing aarch64-linux-gnu-gcc` | `sudo apt install gcc-aarch64-linux-gnu` |
| `KERNEL_SRC` not set | Clone [raspberrypi/linux](https://github.com/raspberrypi/linux) to `kernel/linux-rpi` or set `KERNEL_SRC` |
| `mk-pi-image` permission errors | Run `mk-pi-rootfs.sh` as user first; only image step needs `sudo` |
| Pi shows “Firmware not found” / no boot | Run `./scripts/fetch-pi-firmware.sh` before `mk-pi-image.sh` |
| `aarch64-linux-musl-gcc` not found | `mk-pi-rootfs.sh` auto-downloads from musl.cc, or install a cross toolchain |
| Pi boots but no network | Cable, DHCP server; `ifconfig eth0` on the HDMI shell |
| No picture on HDMI | Check cable; `hdmi_force_hotplug=1` is in `config.txt` |
| USB keyboard has no effect | Try another USB port; kernel needs USB + HID (see `kernel/config.pi4`) |
| `thttpd` / `dnscached` fail to start | `/etc/passwd` must include `http` and `dnscache` users |
| Kernel panic: unable to mount root | `root=/dev/mmcblk0p2` in `cmdline.txt`; reflash full `sdcard.img` |

---

## Related docs

- [QEMU.md](QEMU.md) — x86_64 initramfs development image
- [README.md](../README.md) — RustBox build and applet configuration
- [SECURITY.md](SECURITY.md) — `thttpd`, `dnscached`, `telnetd` exposure notes
