mod support;

use support::{Rustbox, TestDir};

#[test]
fn head_prints_first_lines() {
    let dir = TestDir::new();
    dir.write("lines.txt", "one\ntwo\nthree\nfour\n");
    assert_eq!(
        Rustbox::new()
            .applet("head")
            .args(["-n", "2"])
            .arg(dir.join("lines.txt"))
            .stdout(),
        "one\ntwo\n"
    );
}

#[test]
fn tail_prints_last_lines() {
    let dir = TestDir::new();
    dir.write("lines.txt", "one\ntwo\nthree\nfour\n");
    assert_eq!(
        Rustbox::new()
            .applet("tail")
            .args(["-n", "2"])
            .arg(dir.join("lines.txt"))
            .stdout(),
        "three\nfour\n"
    );
}

#[test]
fn wc_counts_lines_words_bytes() {
    let dir = TestDir::new();
    dir.write("count.txt", "hello world\n");
    let out = Rustbox::new()
        .applet("wc")
        .arg(dir.join("count.txt"))
        .stdout();
    assert!(out.contains("1"));
    assert!(out.contains("2"));
    assert!(out.contains("count.txt"));
}

#[test]
fn ls_lists_directory() {
    let dir = TestDir::new();
    dir.write("visible.txt", "");
    let out = Rustbox::new().applet("ls").arg(dir.path()).stdout();
    assert!(out.contains("visible.txt"));
}

#[test]
fn ls_long_format() {
    let dir = TestDir::new();
    dir.write("file.txt", "data");
    let out = Rustbox::new()
        .applet("ls")
        .args(["-l", dir.join("file.txt").to_str().unwrap()])
        .stdout();
    assert!(out.contains("file.txt"));
    assert!(out.starts_with('-'));
}
