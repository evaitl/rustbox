mod support;

use support::Rustbox;

fn has_net_admin() -> bool {
    if !cfg!(target_os = "linux") || unsafe { libc::geteuid() } != 0 {
        return false;
    }
    // Root in a sandbox or container may lack CAP_NET_ADMIN; probe instead of assuming.
    Rustbox::new()
        .applet("ifconfig")
        .args(["lo", "127.0.0.1", "up"])
        .status()
        == 0
}

#[test]
fn hostname_get_and_set() {
    if !cfg!(target_os = "linux") || !has_net_admin() {
        return;
    }
    let before = Rustbox::new().applet("hostname").stdout();
    let before = before.trim().to_string();

    assert_eq!(
        Rustbox::new()
            .applet("hostname")
            .arg("rustbox-test")
            .status(),
        0
    );

    let after = Rustbox::new().applet("hostname").stdout();
    assert_eq!(after.trim(), "rustbox-test");

    assert_eq!(Rustbox::new().applet("hostname").arg(&before).status(), 0);
}

#[test]
fn ifconfig_lo_up() {
    if !cfg!(target_os = "linux") || !has_net_admin() {
        return;
    }
    assert_eq!(
        Rustbox::new()
            .applet("ifconfig")
            .args(["lo", "127.0.0.1", "up"])
            .status(),
        0
    );

    let out = Rustbox::new().applet("ifconfig").arg("lo").stdout();
    assert!(out.contains("inet 127.0.0.1"), "output: {out}");
}

#[test]
fn route_show_table() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("route").arg("-n").stdout();
    assert!(out.contains("Destination"));
}

#[test]
fn ping_localhost() {
    if !cfg!(target_os = "linux") || !has_net_admin() {
        return;
    }
    assert_eq!(
        Rustbox::new()
            .applet("ping")
            .args(["-c", "1", "-W", "2", "-q", "127.0.0.1"])
            .status(),
        0
    );
}
