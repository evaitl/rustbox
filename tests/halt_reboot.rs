mod support;

use support::Rustbox;

#[test]
fn halt_unknown_option_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("halt")
            .arg("--not-an-option")
            .status(),
        1
    );
}

#[test]
fn reboot_unknown_option_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("reboot")
            .arg("--not-an-option")
            .status(),
        1
    );
}

#[test]
fn halt_without_privilege_fails() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    assert_eq!(Rustbox::new().applet("halt").status(), 1);
}

#[test]
fn reboot_without_privilege_fails() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    assert_eq!(Rustbox::new().applet("reboot").status(), 1);
}
