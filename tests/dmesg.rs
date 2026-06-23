mod support;

use support::Rustbox;

#[test]
fn dmesg_prints_kernel_log() {
    let out = Rustbox::new().applet("dmesg").output();
    if out.status.success() {
        return;
    }
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("Operation not permitted") || err.contains("Permission denied"),
        "dmesg failed unexpectedly: {err}"
    );
}

#[test]
fn dmesg_rejects_unknown_option() {
    assert_eq!(Rustbox::new().applet("dmesg").arg("-z").status(), 1);
}
