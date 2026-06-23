mod support;

use support::{bin, Rustbox, TestDir};

#[test]
fn init_oneshot_runs_sysinit_and_once() {
    let dir = TestDir::new();
    let bin_path = bin();
    let bin = bin_path.to_str().expect("utf-8 path");
    dir.write(
        "inittab",
        &format!("::sysinit:{bin} echo sysinit\n::once:{bin} echo once\n"),
    );

    let out = Rustbox::new()
        .applet("init")
        .arg("-f")
        .arg(dir.join("inittab"))
        .arg("-s")
        .stdout();

    assert!(out.contains("sysinit"));
    assert!(out.contains("once"));
}

#[test]
fn init_wait_action_blocks_until_complete() {
    let dir = TestDir::new();
    let bin_path = bin();
    let bin = bin_path.to_str().expect("utf-8 path");
    dir.write("inittab", &format!("::wait:{bin} echo waited\n"));

    let out = Rustbox::new()
        .applet("init")
        .arg("-f")
        .arg(dir.join("inittab"))
        .arg("-s")
        .stdout();

    assert!(out.contains("waited"));
}

#[test]
fn init_missing_inittab_fails() {
    assert_eq!(
        Rustbox::new()
            .applet("init")
            .arg("-f")
            .arg("/no/such/inittab")
            .status(),
        1
    );
}
