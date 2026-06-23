//! Integration tests for the `vi` applet using scripted key fixtures.
//!
//! Each case under `tests/vi_fixtures/<name>/` contains:
//! - `input.txt` — starting file contents
//! - `keys.txt` — key script (`<Esc>`, `<Enter>`, arrow tokens, …)
//! - `expected.txt` — file contents after a successful edit session
//! - `exit` (optional) — expected exit code (default `0`)

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use support::{Rustbox, TestDir};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vi_fixtures")
}

fn run_fixture(case_dir: &Path) {
    let name = case_dir.file_name().unwrap().to_string_lossy().into_owned();

    let mut input = fs::read_to_string(case_dir.join("input.txt"))
        .unwrap_or_else(|e| panic!("{name}: read input.txt: {e}"));
    if input.ends_with('\n') {
        input.pop();
        if input.ends_with('\r') {
            input.pop();
        }
    }
    let keys_path = case_dir.join("keys.txt");
    let mut expected = fs::read_to_string(case_dir.join("expected.txt"))
        .unwrap_or_else(|e| panic!("{name}: read expected.txt: {e}"));
    if expected.ends_with('\n') {
        expected.pop();
        if expected.ends_with('\r') {
            expected.pop();
        }
    }
    let want_exit: i32 = fs::read_to_string(case_dir.join("exit"))
        .ok()
        .map(|s| s.trim().parse().expect("invalid exit file"))
        .unwrap_or(0);

    let dir = TestDir::new();
    let file = dir.write("file.txt", &input);

    let status = Rustbox::new()
        .applet("vi")
        .arg("-T")
        .arg(&keys_path)
        .arg(&file)
        .status();

    assert_eq!(status, want_exit, "case {name}: exit status");

    if want_exit == 0 {
        let got = dir.read("file.txt");
        assert_eq!(got, expected, "case {name}: file contents");
    }
}

#[test]
fn vi_fixtures() {
    let root = fixtures_root();
    let mut cases: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("read {}: {e}", root.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    cases.sort_by_key(|e| e.file_name());

    assert!(!cases.is_empty(), "no vi fixtures under {}", root.display());

    for entry in cases {
        run_fixture(&entry.path());
    }
}

#[test]
fn vi_lists_in_applets() {
    let out = Rustbox::new().arg("--list").stdout();
    assert!(out.lines().any(|l| l == "vi"), "vi missing from --list");
}
