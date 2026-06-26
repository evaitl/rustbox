# Testing

RustBox uses several layers of verification: fast Rust unit and integration tests for day-to-day development, libFuzzer campaigns for parser and protocol code, and QEMU-based boot tests for the full initrd image. This document summarizes what each layer covers and how to run it.

## Quick reference

| Layer | Command | Typical use |
|-------|---------|-------------|
| Format | `cargo fmt --check` | Before every commit |
| Lint | `cargo clippy --all-targets` | Before every commit |
| Unit / integration | `cargo test` | Every change |
| Dependency audit | `cargo audit` | Before releases; after dependency bumps |
| Local CI bundle | `./scripts/ci-local.sh` | Match GitHub Actions locally |
| QEMU smoke | `./scripts/qemu-smoke.sh` | Initrd / networking regression |
| QEMU soak | `./scripts/qemu-soak.sh` | Long-run stability and memory |
| Fuzz (single target) | `cargo +nightly fuzz run <target> …` | Targeted fuzzing |
| Fuzz regression | `./scripts/fuzz-regression.sh <target>` | Replay saved crash inputs |
| Fuzz + coverage | `./scripts/fuzz-coverage.sh` | Coverage-guided fuzz workflow |
| Fuzz campaign | `./scripts/fuzz-campaign.sh` | Extended multi-target run (~30 min each) |

Continuous integration (`.github/workflows/ci.yml`) runs `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` on every push and pull request to `main`. Fuzzing, QEMU smoke/soak, and `cargo audit` are run locally or on a schedule — they are not in CI today.

---

## Static checks

### `cargo fmt`

Enforces consistent formatting. CI fails if the tree is not formatted.

```bash
cargo fmt              # apply
cargo fmt --check      # verify only (CI)
```

### `cargo clippy`

Runs the Clippy linter on all targets (library, binaries, tests, benches).

```bash
cargo clippy --all-targets
```

Fix warnings before merging when practical; CI treats Clippy output as errors.

### `cargo audit`

Scans `Cargo.lock` against the [RustSec advisory database](https://rustsec.org/). Recommended before releases and after changing dependencies. See also [SECURITY.md](SECURITY.md).

```bash
cargo install cargo-audit   # once
cargo audit
```

---

## `cargo test`

The primary correctness gate. Tests live in:

- **`src/`** — unit tests inside modules (`#[cfg(test)]`), including fuzz-only harness tests behind the `fuzzing` feature.
- **`tests/`** — integration tests that build the `rustbox` binary and exercise applets end-to-end in a temporary workspace (shell, networking, `vi`, `thttpd`, `init`, and others).

```bash
cargo test
```

Run a single integration test file:

```bash
cargo test --test net
```

Some tests are feature-gated (for example `applet-sshd`) or marked `#[ignore]`; see [README.md](../README.md) for SSHD password-hash helpers.

---

## Local CI script

[`scripts/ci-local.sh`](../scripts/ci-local.sh) mirrors the GitHub workflow: format check, Clippy, and tests. It pins the **stable** toolchain on `PATH` so a default nightly install does not break the musl initrd build used by smoke tests.

```bash
./scripts/ci-local.sh
```

Optional QEMU smoke (needs `qemu-system-x86_64`, kernel at `kernel/vmlinuz`, and build tools for `mkinitrd.sh`):

```bash
CI_LOCAL_SMOKE=1 ./scripts/ci-local.sh
```

---

## Fuzz testing

Fuzz targets live under [`fuzz/`](../fuzz/) and use [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) with libFuzzer. **A nightly Rust toolchain is required** for fuzz builds; normal `cargo build` stays on stable.

| Target | Exercises |
|--------|-----------|
| `rash_parse` | Shell parser |
| `rash_arith` | Arithmetic expansion |
| `rash_run` | Builtin-only script execution (`PATH` cleared in the harness) |
| `thttpd` | Config and HTTP parsing |
| `udhcpc` | DHCP client arguments |
| `wget` | URL / request parsing |
| `dnscached` | DNS cache protocol |
| `sshd` | SSH parsing (when enabled in fuzz config) |

Setup:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
./scripts/mk-fuzz-corpus.sh    # seed corpora under fuzz/corpus/<target>/ (gitignored)
```

Run one target for a short session:

```bash
cd fuzz
cargo +nightly fuzz run rash_parse fuzz/corpus/rash_parse -- -max_total_time=60
cargo +nightly fuzz run rash_run fuzz/corpus/rash_run -- -timeout=5 -max_total_time=60
```

[`scripts/fuzz-env.sh`](../scripts/fuzz-env.sh) is sourced by fuzz helper scripts to select nightly and a dedicated applet config (`fuzz/applets-fuzz.json`). Fuzz scripts restore your normal `PATH` when finished.

**Artifacts:** crashes and timeouts are written to `fuzz/artifacts/<target>/` (gitignored). **`rash_run`** uses a 5-second per-input timeout to catch hangs.

**Watchdog:** long fuzz jobs can be wrapped with [`scripts/fuzz-watchdog.sh`](../scripts/fuzz-watchdog.sh), which kills the child if the log file stops growing (stall detection).

### Fuzz regression

Replay every saved artifact for a target and fail if any input still crashes or times out:

```bash
./scripts/fuzz-regression.sh rash_run
./scripts/fuzz-regression.sh rash_parse
```

### Fuzz testing with coverage

[`scripts/fuzz-coverage.sh`](../scripts/fuzz-coverage.sh) runs a three-phase workflow per target:

1. **Fuzz** — libFuzzer for `FUZZ_DURATION` seconds (default 60).
2. **Coverage build** — rebuild the fuzzer with `-Cinstrument-coverage`.
3. **Replay** — run corpus + artifacts with `-runs=0` to emit `*.profraw` files.

Profiles are merged to `fuzz/<target>.profdata`. Generate reports with `llvm-cov` (requires LLVM tools, e.g. `apt install llvm`):

```bash
llvm-cov report fuzz/target/<host-triple>/release/rash_parse \
  -instr-profile=fuzz/rash_parse.profdata \
  ../src/applets/sh/parse.rs
```

Override targets or duration:

```bash
FUZZ_TARGETS="rash_parse rash_run" FUZZ_DURATION=300 ./scripts/fuzz-coverage.sh
```

### Extended fuzz campaign

[`scripts/fuzz-campaign.sh`](../scripts/fuzz-campaign.sh) runs each major target for **30 minutes** (rash split across parse / arith / run). Logs go to `/tmp/rustbox-fuzz-logs/` by default.

---

## QEMU smoke test

End-to-end boot test: builds a musl initrd, boots QEMU with virtio-net, runs [`initrd/template/sbin/smoke-test`](../initrd/template/sbin/smoke-test) inside the guest, and exits when the serial log contains `smoke: ok`.

```bash
./scripts/qemu-smoke.sh
```

Covers shell builtins, `cron`, networking (`udhcpc`, `ping`, `route`), `thttpd`, `wget`, `dig`, `logger`, `ntpclient`, `nc`, and more. Default wall-clock limit: **60 seconds** (`SMOKE_TIMEOUT`).

Full prerequisites, kernel build, and environment variables: **[QEMU.md](QEMU.md)**.

---

## QEMU soak test

Long-running stability test after smoke passes. [`scripts/qemu-soak.sh`](../scripts/qemu-soak.sh) keeps QEMU up while [`initrd/template/sbin/soak-loop`](../initrd/template/sbin/soak-loop) issues periodic `wget` / `dig` / `ping` load and prints heartbeats with guest memory usage (`/proc/meminfo`) and daemon RSS (`/proc/*/status`).

Defaults:

- **Duration:** 12 hours (`SOAK_DURATION=43200`)
- **Checks:** every 15 minutes (`CHECK_INTERVAL=900`) for hung heartbeats and memory growth

```bash
./scripts/qemu-soak.sh

# Shorter local run
SOAK_DURATION=3600 CHECK_INTERVAL=300 ./scripts/qemu-soak.sh

# Overnight in background
nohup ./scripts/qemu-soak.sh > /tmp/qemu-soak-host.log 2>&1 &
```

Metrics and serial output: `initrd/qemu-soak.metrics`, `initrd/qemu-soak.log`. Details: **[QEMU.md](QEMU.md#qemu-soak-test)**.

---

## Manual QEMU testing

For interactive debugging without the boot-time smoke test:

```bash
./scripts/qemu-shell.sh
```

Brings up a serial `rash` shell with networking; useful for reproducing guest issues by hand. See [QEMU.md](QEMU.md#interactive-shell-qemu-shellsh).

---

## Suggested workflow

| When | What to run |
|------|-------------|
| Every edit | `cargo test` (or `./scripts/ci-local.sh`) |
| Before opening a PR | `cargo fmt`, `cargo clippy --all-targets`, `cargo test` |
| Parser / shell / HTTP changes | Short fuzz on the relevant target; `fuzz-regression.sh` if artifacts exist |
| Initrd, init, or applet wiring | `qemu-smoke.sh` |
| Suspected leaks or daemon stalls | `qemu-soak.sh` (or a shortened `SOAK_DURATION`) |
| Dependency or release prep | `cargo audit` |
| Deep coverage investigation | `fuzz-coverage.sh` + `llvm-cov report` |

Fuzzing and QEMU tests need extra tooling (nightly, QEMU, kernel build, `cpio`, musl target). Install steps are in [README.md](../README.md) and [QEMU.md](QEMU.md).
