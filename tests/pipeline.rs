mod support;

use support::{Rustbox, TestDir};

#[test]
fn cut_fields() {
    let dir = TestDir::new();
    let path = dir.write("input.txt", "a:b:c\n");
    let out = Rustbox::new()
        .applet("cut")
        .args(["-d:", "-f2"])
        .arg(path)
        .stdout();
    assert_eq!(out.trim(), "b");
}

#[test]
fn tr_uppercase() {
    let dir = TestDir::new();
    let path = dir.write("input.txt", "abc\n");
    let out = Rustbox::new()
        .applet("tr")
        .args(["a-z", "A-Z"])
        .arg(path)
        .stdout();
    assert_eq!(out.trim(), "ABC");
}

#[test]
fn sort_lines() {
    let dir = TestDir::new();
    let path = dir.write("input.txt", "b\na\nc\n");
    let out = Rustbox::new().applet("sort").arg(path).stdout();
    assert_eq!(out.trim(), "a\nb\nc");
}

#[test]
fn printenv_path() {
    let out = Rustbox::new().applet("printenv").arg("PATH").stdout();
    assert!(!out.trim().is_empty());
}

#[test]
fn env_runs_command() {
    let bin = support::bin();
    let bin = bin.to_str().expect("utf-8 path");
    let status = Rustbox::new().applet("env").args([bin, "false"]).status();
    assert_ne!(status, 0);
}

#[test]
fn histfile_persists_history() {
    let dir = TestDir::new();
    let hist = dir.join("history");
    let hist_str = hist.to_str().expect("utf-8 path");
    let _ = Rustbox::new()
        .applet("rash")
        .args(["-i"])
        .env("HISTFILE", hist_str)
        .stdin_input("echo saved\n")
        .status();
    let contents = dir.read("history");
    assert!(contents.contains("echo saved"));
}
