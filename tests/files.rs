mod support;

use support::{Rustbox, TestDir};

#[test]
fn mkdir_creates_directory() {
    let dir = TestDir::new();
    let target = dir.join("sub");
    assert_eq!(Rustbox::new().applet("mkdir").arg(&target).status(), 0);
    assert!(target.is_dir());
}

#[test]
fn mkdir_parents_creates_nested_directories() {
    let dir = TestDir::new();
    let target = dir.join("a/b/c");
    assert_eq!(
        Rustbox::new()
            .applet("mkdir")
            .args(["-p", target.to_str().unwrap()])
            .status(),
        0
    );
    assert!(target.is_dir());
}

#[test]
fn cp_copies_file() {
    let dir = TestDir::new();
    dir.write("src.txt", "copy me");
    let dst = dir.join("dst.txt");
    assert_eq!(
        Rustbox::new()
            .applet("cp")
            .args([dir.join("src.txt"), dst.clone()])
            .status(),
        0
    );
    assert_eq!(dir.read("dst.txt"), "copy me");
}

#[test]
fn cp_recursive_copies_directory() {
    let dir = TestDir::new();
    dir.write("tree/a.txt", "a");
    dir.write("tree/b.txt", "b");
    let dst = dir.join("tree-copy");
    assert_eq!(
        Rustbox::new()
            .applet("cp")
            .arg("-r")
            .arg(dir.join("tree"))
            .arg(dst.clone())
            .status(),
        0
    );
    assert_eq!(dir.read("tree-copy/a.txt"), "a");
    assert_eq!(dir.read("tree-copy/b.txt"), "b");
}

#[test]
fn mv_moves_file() {
    let dir = TestDir::new();
    dir.write("old.txt", "move me");
    let new_path = dir.join("new.txt");
    assert_eq!(
        Rustbox::new()
            .applet("mv")
            .args([dir.join("old.txt"), new_path.clone()])
            .status(),
        0
    );
    assert!(!dir.exists("old.txt"));
    assert_eq!(dir.read("new.txt"), "move me");
}

#[test]
fn rm_removes_file() {
    let dir = TestDir::new();
    dir.write("gone.txt", "x");
    assert_eq!(
        Rustbox::new()
            .applet("rm")
            .arg(dir.join("gone.txt"))
            .status(),
        0
    );
    assert!(!dir.exists("gone.txt"));
}

#[test]
fn rm_recursive_removes_directory() {
    let dir = TestDir::new();
    dir.write("dir/x.txt", "x");
    assert_eq!(
        Rustbox::new()
            .applet("rm")
            .arg("-r")
            .arg(dir.join("dir"))
            .status(),
        0
    );
    assert!(!dir.exists("dir"));
}

#[test]
fn ln_hard_link() {
    let dir = TestDir::new();
    dir.write("orig.txt", "linked");
    let link = dir.join("hard.txt");
    assert_eq!(
        Rustbox::new()
            .applet("ln")
            .args([dir.join("orig.txt"), link.clone()])
            .status(),
        0
    );
    assert_eq!(dir.read("hard.txt"), "linked");
}

#[test]
fn ln_symbolic_link() {
    let dir = TestDir::new();
    dir.write("orig.txt", "sym");
    let link = dir.join("sym.txt");
    assert_eq!(
        Rustbox::new()
            .applet("ln")
            .args(["-s", "orig.txt", link.to_str().unwrap()])
            .current_dir(dir.path())
            .status(),
        0
    );
    assert_eq!(dir.read("sym.txt"), "sym");
}

#[test]
fn mknod_creates_fifo() {
    use std::os::unix::fs::FileTypeExt;
    let dir = TestDir::new();
    let fifo = dir.join("fifo");
    assert_eq!(
        Rustbox::new()
            .applet("mknod")
            .args([fifo.to_str().unwrap(), "p"])
            .status(),
        0
    );
    assert!(std::fs::metadata(&fifo).unwrap().file_type().is_fifo());
}

#[test]
fn rmdir_removes_empty_directory() {
    let dir = TestDir::new();
    let sub = dir.join("empty");
    std::fs::create_dir(&sub).unwrap();
    assert_eq!(Rustbox::new().applet("rmdir").arg(&sub).status(), 0);
    assert!(!sub.exists());
}

#[test]
fn rmdir_fails_on_nonempty_directory() {
    let dir = TestDir::new();
    dir.write("dir/x.txt", "x");
    assert_ne!(
        Rustbox::new().applet("rmdir").arg(dir.join("dir")).status(),
        0
    );
}
