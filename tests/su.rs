mod support;

use support::Rustbox;

#[test]
fn su_help() {
    let status = Rustbox::new().applet("su").args(["--help"]).status();
    assert_eq!(status, 0);
}

#[test]
fn su_lists_in_applets() {
    let out = Rustbox::new().args(["--list"]).stdout();
    assert!(out.lines().any(|line| line.trim() == "su"));
}

#[test]
fn su_requires_user() {
    let out = Rustbox::new().applet("su").stderr();
    assert!(out.contains("missing USER"));
    assert_eq!(Rustbox::new().applet("su").status(), 1);
}

#[test]
fn su_rejects_non_root() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    let out = Rustbox::new()
        .applet("su")
        .args(["nobody", "-c", "true"])
        .stderr();
    assert!(out.contains("must be suid"));
}

#[test]
fn su_unknown_user_fails() {
    if !rustix::process::geteuid().is_root() {
        return;
    }
    let out = Rustbox::new()
        .applet("su")
        .args(["no-such-rustbox-user-xyz", "-c", "true"])
        .stderr();
    assert!(out.contains("unknown user"));
}
