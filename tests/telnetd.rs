mod support;

use support::Rustbox;

#[test]
fn telnetd_help() {
    let status = Rustbox::new().applet("telnetd").args(["--help"]).status();
    assert_eq!(status, 0);
}

#[test]
fn telnetd_lists_in_applets() {
    let out = Rustbox::new().args(["--list"]).stdout();
    assert!(out.lines().any(|line| line.trim() == "telnetd"));
}

#[test]
fn telnetd_config_parse() {
    let cfg = rustbox::net::telnetd::parse_config_text("listen 127.0.0.1\nport 2323\n");
    assert_eq!(cfg.listen_addr, "127.0.0.1");
    assert_eq!(cfg.port, 2323);
}
