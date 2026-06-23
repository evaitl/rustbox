mod support;

use support::Rustbox;

#[test]
fn prints_arguments() {
    assert_eq!(
        Rustbox::new()
            .applet("echo")
            .args(["a", "b"])
            .stdout()
            .trim(),
        "a b"
    );
}

#[test]
fn empty_prints_newline() {
    assert_eq!(Rustbox::new().applet("echo").stdout(), "\n");
}

#[test]
fn no_newline_flag() {
    assert_eq!(
        Rustbox::new().applet("echo").args(["-n", "x"]).stdout(),
        "x"
    );
}
