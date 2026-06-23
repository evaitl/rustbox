mod support;

use support::Rustbox;

#[test]
fn thttpd_help() {
    let status = Rustbox::new().applet("thttpd").args(["--help"]).status();
    assert_eq!(status, 0);
}

#[test]
fn thttpd_lists_in_applets() {
    let out = Rustbox::new().args(["--list"]).stdout();
    assert!(out.lines().any(|line| line.trim() == "thttpd"));
}
