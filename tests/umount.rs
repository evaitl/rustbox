mod support;

use support::Rustbox;

#[test]
fn umount_missing_operand_fails() {
    assert_eq!(Rustbox::new().applet("umount").status(), 1);
}

#[test]
fn umount_unknown_option_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("umount")
            .args(["--not-an-option"])
            .status(),
        1
    );
}

#[test]
fn umount_non_mountpoint_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("umount")
            .arg("/tmp/rustbox-umount-test-nonexistent")
            .status(),
        1
    );
}
