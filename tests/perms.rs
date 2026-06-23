mod support;

use std::os::unix::fs::PermissionsExt;
use support::{Rustbox, TestDir};

#[test]
fn chmod_sets_mode() {
    let dir = TestDir::new();
    let path = dir.join("file.txt");
    dir.write("file.txt", "x");
    assert_eq!(
        Rustbox::new()
            .applet("chmod")
            .args(["600", path.to_str().unwrap()])
            .status(),
        0
    );
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn chmod_recursive_sets_mode() {
    let dir = TestDir::new();
    dir.write("tree/a.txt", "a");
    let tree = dir.join("tree");
    assert_eq!(
        Rustbox::new()
            .applet("chmod")
            .args(["-R", "700", tree.to_str().unwrap()])
            .status(),
        0
    );
    let mode = std::fs::metadata(tree.join("a.txt"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
}

#[test]
fn chown_sets_uid() {
    let dir = TestDir::new();
    let path = dir.join("owned.txt");
    dir.write("owned.txt", "x");
    let uid = rustix::process::getuid().as_raw();
    assert_eq!(
        Rustbox::new()
            .applet("chown")
            .args([&uid.to_string(), path.to_str().unwrap()])
            .status(),
        0
    );
}

#[test]
fn stat_prints_file_info() {
    let dir = TestDir::new();
    dir.write("info.txt", "hello");
    let out = Rustbox::new()
        .applet("stat")
        .arg(dir.join("info.txt"))
        .stdout();
    assert!(out.contains("File:"));
    assert!(out.contains("Size:"));
}

#[test]
fn stat_format_mode() {
    let dir = TestDir::new();
    dir.write("mode.txt", "x");
    let file = dir.join("mode.txt");
    dir.write("mode.txt", "x");
    let out = Rustbox::new()
        .applet("stat")
        .args(["-c", "%a %n", file.to_str().unwrap()])
        .stdout();
    assert!(out.contains("mode.txt"));
}

#[test]
fn test_file_exists() {
    let dir = TestDir::new();
    dir.write("present.txt", "x");
    assert_eq!(
        Rustbox::new()
            .applet("test")
            .args(["-f", dir.join("present.txt").to_str().unwrap()])
            .status(),
        0
    );
    assert_eq!(
        Rustbox::new()
            .applet("test")
            .args(["-f", dir.join("missing.txt").to_str().unwrap()])
            .status(),
        1
    );
}

#[test]
fn test_string_equal() {
    assert_eq!(
        Rustbox::new().applet("test").args(["a", "=", "a"]).status(),
        0
    );
    assert_eq!(
        Rustbox::new().applet("test").args(["a", "=", "b"]).status(),
        1
    );
}

#[test]
fn test_bracket_form() {
    assert_eq!(
        Rustbox::new().applet("[").args(["-n", "x", "]"]).status(),
        0
    );
}
