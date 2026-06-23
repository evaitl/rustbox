mod support;

use support::{Rustbox, TestDir};

#[test]
fn help_exits_zero() {
    assert_eq!(Rustbox::new().args(["--help"]).status(), 0);
}

#[test]
fn list_includes_echo() {
    let out = Rustbox::new().args(["--list"]).stdout();
    assert!(out.lines().any(|line| line == "echo"));
}

#[test]
fn unknown_applet_exits_127() {
    assert_eq!(Rustbox::new().applet("not-an-applet").status(), 127);
}

#[test]
fn symlink_invocation() {
    let dir = TestDir::new();
    let link = dir.join("echo");
    std::os::unix::fs::symlink(support::bin(), &link).expect("symlink");

    let out = std::process::Command::new(&link)
        .arg("hello")
        .output()
        .expect("run symlink");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}
