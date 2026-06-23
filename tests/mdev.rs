use support::Rustbox;

mod support;

#[test]
fn mdev_help() {
    let status = Rustbox::new().applet("mdev").arg("--help").status();
    assert_eq!(status, 0);
}

#[test]
fn mdev_lists_in_applets() {
    let out = Rustbox::new().args(["--list"]).stdout();
    assert!(out.lines().any(|line| line == "mdev"));
}

#[test]
fn mdev_scan_exits_zero() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let status = Rustbox::new().applet("mdev").arg("-s").status();
    assert_eq!(status, 0);
}
