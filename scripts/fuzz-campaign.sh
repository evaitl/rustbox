#!/usr/bin/env bash
# Run libFuzzer targets for 30 minutes each (rash split across parse/arith/run).
set -euo pipefail
cd "$(dirname "$0")/../fuzz"

# rustup shims can be silent no-ops in some environments; use the toolchain directly.
if [[ -d "${HOME}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin" ]]; then
  export PATH="${HOME}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin:${PATH}"
fi
unset CARGO_TARGET_DIR RUSTFLAGS

DURATION=1800
LOG_DIR="${LOG_DIR:-/tmp/rustbox-fuzz-logs}"
mkdir -p "$LOG_DIR"

run_one() {
  local name=$1
  local extra=${2:-}
  echo "=== fuzz $name (${DURATION}s) ===" | tee "$LOG_DIR/$name.log"
  if cargo fuzz run "$name" -- $extra -max_total_time=$DURATION >>"$LOG_DIR/$name.log" 2>&1; then
    echo "fuzz $name: finished" | tee -a "$LOG_DIR/$name.log"
  else
    echo "fuzz $name: exited non-zero (possible crash)" | tee -a "$LOG_DIR/$name.log"
  fi
}


export RUSTBOX_APPLETS_CONFIG="${RUSTBOX_APPLETS_CONFIG:-$(dirname "$0")/../fuzz/applets-fuzz.json}"

run_one dnscached
run_one sshd

run_one wget
run_one udhcpc
run_one thttpd

RASH_DURATION=$((DURATION / 3))
for target in rash_parse rash_arith rash_run; do
  echo "=== fuzz $target (${RASH_DURATION}s) ===" | tee "$LOG_DIR/$target.log"
  extra=""
  [[ $target == rash_run ]] && extra="-timeout=5"
  if cargo fuzz run "$target" -- $extra -max_total_time=$RASH_DURATION >>"$LOG_DIR/$target.log" 2>&1; then
    echo "fuzz $target: finished" | tee -a "$LOG_DIR/$target.log"
  else
    echo "fuzz $target: exited non-zero (possible crash)" | tee -a "$LOG_DIR/$target.log"
  fi
done

echo "=== fuzz campaign complete ===" | tee "$LOG_DIR/summary.log"
find ../fuzz/artifacts -type f 2>/dev/null | tee -a "$LOG_DIR/summary.log" || true
