mod support;

use support::Rustbox;

#[test]
fn mount_lists_mount_table() {
    let out = Rustbox::new().applet("mount").stdout();
    assert!(out
        .lines()
        .next()
        .is_some_and(|line| !line.trim().is_empty()));
}

#[test]
fn mount_unknown_option_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("mount")
            .args(["-o", "not-a-real-option", "/mnt"])
            .status(),
        1
    );
}

#[test]
fn mount_missing_target_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("mount")
            .args(["-o", "remount"])
            .status(),
        1
    );
}
