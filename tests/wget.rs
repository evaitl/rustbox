mod support;

use support::Rustbox;

#[test]
fn wget_help() {
    let status = Rustbox::new().applet("wget").args(["--help"]).status();
    assert_eq!(status, 0);
}

#[test]
fn wget_lists_in_applets() {
    let out = Rustbox::new().args(["--list"]).stdout();
    assert!(out.lines().any(|line| line.trim() == "wget"));
}

#[test]
fn wget_rejects_non_http_url() {
    let status = Rustbox::new()
        .applet("wget")
        .arg("ftp://example.com/")
        .status();
    assert_eq!(status, 1);
}

#[cfg(not(feature = "wget-tls"))]
#[test]
fn wget_rejects_https_without_tls_feature() {
    let out = Rustbox::new()
        .applet("wget")
        .arg("https://127.0.0.1/")
        .stderr();
    assert!(out.contains("wget-tls"));
}
