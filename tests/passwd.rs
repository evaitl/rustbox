mod support;

use support::Rustbox;

#[test]
fn passwd_help() {
    let status = Rustbox::new().applet("passwd").args(["--help"]).status();
    assert_eq!(status, 0);
}

#[test]
fn passwd_lists_in_applets() {
    let out = Rustbox::new().args(["--list"]).stdout();
    assert!(out.lines().any(|line| line.trim() == "passwd"));
}

#[test]
fn passwd_unknown_user_fails() {
    let out = Rustbox::new()
        .applet("passwd")
        .args(["-f", "/nonexistent/passwd", "nobody"])
        .stderr();
    assert!(out.contains("does not exist"));
    assert_eq!(
        Rustbox::new()
            .applet("passwd")
            .args(["-f", "/nonexistent/passwd", "nobody"])
            .status(),
        1
    );
}
