mod support;

use support::Rustbox;

#[test]
fn switch_root_missing_args_fails() {
    assert_eq!(Rustbox::new().applet("switch_root").status(), 1);
}

#[test]
fn switch_root_not_enough_args_fails() {
    assert_eq!(Rustbox::new().applet("switch_root").arg("/tmp").status(), 1);
}

#[test]
fn switch_root_empty_newroot_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("switch_root")
            .args(["", "/sbin/init"])
            .status(),
        1
    );
}
