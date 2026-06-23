mod support;

use support::{Rustbox, TestDir};

#[test]
fn basename_strips_directory() {
    assert_eq!(
        Rustbox::new()
            .applet("basename")
            .arg("/usr/bin/ls")
            .stdout(),
        "ls\n"
    );
}

#[test]
fn basename_suffix_option() {
    assert_eq!(
        Rustbox::new()
            .applet("basename")
            .args(["-s", ".txt", "file.txt"])
            .stdout(),
        "file\n"
    );
}

#[test]
fn dirname_strips_filename() {
    assert_eq!(
        Rustbox::new().applet("dirname").arg("/usr/bin/ls").stdout(),
        "/usr/bin\n"
    );
}

#[test]
fn dirname_root() {
    assert_eq!(Rustbox::new().applet("dirname").arg("/").stdout(), "/\n");
}

#[test]
fn printf_formats_string_and_number() {
    assert_eq!(
        Rustbox::new()
            .applet("printf")
            .args(["%s %d\n", "hello", "42"])
            .stdout(),
        "hello 42\n"
    );
}

#[test]
fn readlink_prints_symlink_target() {
    let dir = TestDir::new();
    dir.write("target.txt", "data");
    let link = dir.join("link.txt");
    assert_eq!(
        Rustbox::new()
            .applet("ln")
            .args(["-s", "target.txt", link.to_str().unwrap()])
            .current_dir(dir.path())
            .status(),
        0
    );
    assert_eq!(
        Rustbox::new()
            .applet("readlink")
            .arg(link.to_str().unwrap())
            .stdout(),
        "target.txt\n"
    );
}

#[test]
fn readlink_canonicalize() {
    let dir = TestDir::new();
    dir.write("real.txt", "x");
    let link = dir.join("link.txt");
    assert_eq!(
        Rustbox::new()
            .applet("ln")
            .args(["-s", "real.txt", link.to_str().unwrap()])
            .current_dir(dir.path())
            .status(),
        0
    );
    let out = Rustbox::new()
        .applet("readlink")
        .args(["-f", link.to_str().unwrap()])
        .stdout();
    assert!(out.ends_with("real.txt\n"));
}

#[test]
fn dd_copies_bytes() {
    let dir = TestDir::new();
    dir.write("in.bin", "abcdef");
    let out = dir.join("out.bin");
    assert_eq!(
        Rustbox::new()
            .applet("dd")
            .args([
                format!("if={}", dir.join("in.bin").display()),
                format!("of={}", out.display()),
                "bs=3".to_string(),
                "count=1".to_string(),
            ])
            .status(),
        0
    );
    assert_eq!(std::fs::read(&out).unwrap(), b"abc");
}
