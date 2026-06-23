mod support;

use support::{Rustbox, TestDir};

#[test]
fn true_exits_zero() {
    assert_eq!(Rustbox::new().applet("true").status(), 0);
}

#[test]
fn false_exits_one() {
    assert_eq!(Rustbox::new().applet("false").status(), 1);
}

#[test]
fn cat_prints_file() {
    let dir = TestDir::new();
    dir.write("file.txt", "hello\n");
    assert_eq!(
        Rustbox::new()
            .applet("cat")
            .arg(dir.path().join("file.txt"))
            .stdout(),
        "hello\n"
    );
}

#[test]
fn pwd_prints_current_directory() {
    let dir = TestDir::new();
    let out = Rustbox::new()
        .current_dir(dir.path())
        .applet("pwd")
        .stdout();
    assert_eq!(out.trim(), dir.path().to_string_lossy());
}

#[test]
fn sleep_accepts_fractional_seconds() {
    assert_eq!(Rustbox::new().applet("sleep").arg("0.01").status(), 0);
}

#[test]
fn uname_default_prints_sysname() {
    let out = Rustbox::new().applet("uname").stdout();
    assert!(!out.trim().is_empty());
}

#[test]
fn uname_sysname_flag() {
    let out = Rustbox::new().applet("uname").arg("-s").stdout();
    assert!(!out.trim().is_empty());
}
