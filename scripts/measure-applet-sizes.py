#!/usr/bin/env python3
"""Measure marginal stripped musl release size per applet."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-musl"
CARGO_TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", "/tmp/rustbox-applet-sizes"))
BIN = CARGO_TARGET_DIR / TARGET / "release" / "rustbox"

# (documentation name, applet keys to disable)
GROUPS: list[tuple[str, list[str]]] = [
    ("basename", ["basename"]),
    ("cat", ["cat"]),
    ("chmod", ["chmod"]),
    ("chown", ["chown"]),
    ("cp", ["cp"]),
    ("cron", ["cron"]),
    ("cut", ["cut"]),
    ("date", ["date"]),
    ("dd", ["dd"]),
    ("dirname", ["dirname"]),
    ("dnscached", ["dnscached"]),
    ("dmesg", ["dmesg"]),
    ("echo", ["echo"]),
    ("env", ["env"]),
    ("false", ["false"]),
    ("find", ["find"]),
    ("free", ["free"]),
    ("grep", ["grep"]),
    ("halt", ["halt"]),
    ("head", ["head"]),
    ("hostname", ["hostname"]),
    ("ifconfig", ["ifconfig"]),
    ("init", ["init"]),
    ("kill", ["kill"]),
    ("killall", ["killall"]),
    ("ln", ["ln"]),
    ("ls", ["ls"]),
    ("mkdir", ["mkdir"]),
    ("mdev", ["mdev"]),
    ("mknod", ["mknod"]),
    ("mount", ["mount"]),
    ("mv", ["mv"]),
    ("ping", ["ping"]),
    ("pivot_root", ["pivot_root"]),
    ("printenv", ["printenv"]),
    ("printf", ["printf"]),
    ("ps", ["ps"]),
    ("pwd", ["pwd"]),
    ("rash", ["rash", "sh"]),
    ("readlink", ["readlink"]),
    ("reboot", ["reboot"]),
    ("rm", ["rm"]),
    ("rmdir", ["rmdir"]),
    ("route", ["route"]),
    ("sed", ["sed"]),
    ("sleep", ["sleep"]),
    ("sshd", ["sshd"]),
    ("su", ["su"]),
    ("sort", ["sort"]),
    ("stat", ["stat"]),
    ("switch_root", ["switch_root"]),
    ("swapoff", ["swapoff"]),
    ("swapon", ["swapon"]),
    ("sync", ["sync"]),
    ("sysctl", ["sysctl"]),
    ("syslogd", ["syslogd"]),
    ("tail", ["tail"]),
    ("test", ["test", "["]),
    ("thttpd", ["thttpd"]),
    ("top", ["top"]),
    ("tr", ["tr"]),
    ("true", ["true"]),
    ("udhcpc", ["udhcpc"]),
    ("umount", ["umount"]),
    ("uptime", ["uptime"]),
    ("uname", ["uname"]),
    ("vi", ["vi"]),
    ("wc", ["wc"]),
    ("wget", ["wget"]),
    ("xargs", ["xargs"]),
]


def build(config_path: Path) -> int:
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(CARGO_TARGET_DIR)
    env["RUSTFLAGS"] = "-C target-feature=+crt-static"
    env["RUSTBOX_APPLETS_CONFIG"] = str(config_path)
    subprocess.run(
        ["cargo", "build", "--release", "--target", TARGET, "-q"],
        cwd=ROOT,
        env=env,
        check=True,
    )
    return BIN.stat().st_size


def main() -> None:
    CARGO_TARGET_DIR.mkdir(parents=True, exist_ok=True)
    cfg_dir = CARGO_TARGET_DIR / "configs"
    cfg_dir.mkdir(exist_ok=True)

    with (ROOT / "applets.json").open() as f:
        base = json.load(f)

    full_cfg = cfg_dir / "full.json"
    full_cfg.write_text(json.dumps(base))
    print("Building full binary...", file=sys.stderr)
    full = build(full_cfg)
    print(f"full={full}", file=sys.stderr)

    results: dict[str, int] = {}
    for name, keys in GROUPS:
        cfg = json.loads(json.dumps(base))
        for key in keys:
            cfg["applets"][key] = False
        # `rash`/`sh` call into `test_`; measure `test` without the shell.
        if name == "test":
            for key in ("rash", "sh"):
                cfg["applets"][key] = False
        path = cfg_dir / f"without_{name}.json"
        path.write_text(json.dumps(cfg))
        print(f"Building without {name}...", file=sys.stderr)
        without = build(path)
        delta = full - without
        results[name] = max(0, delta)
        print(f"{name} {results[name]}", file=sys.stderr)

    out = cfg_dir / "sizes.json"
    payload = {"full": full, "marginal_bytes": results}
    out.write_text(json.dumps(payload, indent=2))
    data_path = ROOT / "scripts" / "applet-sizes.json"
    data_path.write_text(json.dumps(payload, indent=2))
    print(json.dumps(payload, indent=2))


if __name__ == "__main__":
    main()
