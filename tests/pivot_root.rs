mod support;

use support::Rustbox;

#[test]
fn pivot_root_missing_args_fails() {
    assert_eq!(Rustbox::new().applet("pivot_root").status(), 1);
}

#[test]
fn pivot_root_single_arg_fails() {
    assert_eq!(Rustbox::new().applet("pivot_root").arg("/tmp").status(), 1);
}

#[test]
fn pivot_root_too_many_args_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("pivot_root")
            .args(["/a", "/b", "/c"])
            .status(),
        1
    );
}
