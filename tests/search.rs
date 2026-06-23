mod support;

use support::{Rustbox, TestDir};

#[test]
fn grep_matches_lines() {
    let dir = TestDir::new();
    dir.write("data.txt", "alpha\nbeta\ngamma\n");
    let out = Rustbox::new()
        .applet("grep")
        .args(["beta", dir.join("data.txt").to_str().unwrap()])
        .stdout();
    assert_eq!(out.trim(), "beta");
}

#[test]
fn grep_invert_match() {
    let dir = TestDir::new();
    dir.write("data.txt", "yes\nno\nyes\n");
    let out = Rustbox::new()
        .applet("grep")
        .args(["-v", "yes", dir.join("data.txt").to_str().unwrap()])
        .stdout();
    assert_eq!(out.trim(), "no");
}

#[test]
fn sed_substitutes() {
    let dir = TestDir::new();
    dir.write("data.txt", "hello\nworld\n");
    let out = Rustbox::new()
        .applet("sed")
        .args(["s/o/O/g", dir.join("data.txt").to_str().unwrap()])
        .stdout();
    assert_eq!(out, "hellO\nwOrld\n");
}

#[test]
fn find_by_name() {
    let dir = TestDir::new();
    dir.write("keep.txt", "");
    dir.write("skip.md", "");
    let out = Rustbox::new()
        .applet("find")
        .args([dir.path().to_str().unwrap(), "-name", "*.txt"])
        .stdout();
    assert!(out.contains("keep.txt"));
    assert!(!out.contains("skip.md"));
}

#[test]
fn find_type_file() {
    let dir = TestDir::new();
    dir.write("file.txt", "");
    std::fs::create_dir(dir.join("subdir")).unwrap();
    let out = Rustbox::new()
        .applet("find")
        .args([dir.path().to_str().unwrap(), "-type", "f"])
        .stdout();
    assert!(out.contains("file.txt"));
    assert!(!out.contains("subdir"));
}

#[test]
fn xargs_runs_command() {
    let bin = support::bin();
    let bin = bin.to_str().expect("utf-8 path");
    let status = Rustbox::new()
        .applet("xargs")
        .args([bin, "false"])
        .stdin_input("arg\n")
        .status();
    assert_ne!(status, 0);
}

#[test]
fn xargs_dash_r_skips_empty_stdin() {
    let bin = support::bin();
    let bin = bin.to_str().expect("utf-8 path");
    let status = Rustbox::new()
        .applet("xargs")
        .args(["-r", bin, "false"])
        .stdin_input("")
        .status();
    assert_eq!(status, 0);
}
