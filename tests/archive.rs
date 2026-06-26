mod support;

use std::fs;
use support::{Rustbox, TestDir};

#[test]
fn gzip_roundtrip_file() {
    let dir = TestDir::new();
    let src = dir.path().join("plain.txt");
    fs::write(&src, b"compress me\n").unwrap();
    let gz = dir.path().join("plain.txt.gz");

    assert_eq!(
        Rustbox::new()
            .applet("gzip")
            .arg(src.to_str().unwrap())
            .status(),
        0
    );
    assert!(gz.exists());
    assert!(!src.exists());

    assert_eq!(
        Rustbox::new()
            .applet("gzip")
            .args(["-dc", gz.to_str().unwrap()])
            .stdout()
            .as_bytes(),
        b"compress me\n"
    );

    assert_eq!(
        Rustbox::new()
            .applet("gzip")
            .args(["-d", gz.to_str().unwrap()])
            .status(),
        0
    );
    assert!(src.exists());
    assert_eq!(fs::read(src).unwrap(), b"compress me\n");
}

#[test]
fn tar_create_and_extract() {
    let dir = TestDir::new();
    let data = dir.path().join("data.txt");
    fs::write(&data, b"tar payload\n").unwrap();
    let archive = dir.path().join("test.tar");
    let extract = dir.path().join("out");
    fs::create_dir(&extract).unwrap();

    assert_eq!(
        Rustbox::new()
            .current_dir(dir.path())
            .applet("tar")
            .args([
                "-cf",
                archive.file_name().unwrap().to_str().unwrap(),
                "data.txt",
            ])
            .status(),
        0
    );

    fs::remove_file(&data).unwrap();
    assert_eq!(
        Rustbox::new()
            .current_dir(&extract)
            .applet("tar")
            .args(["-xf", archive.to_str().unwrap(),])
            .status(),
        0
    );
    assert_eq!(
        fs::read(extract.join("data.txt")).unwrap(),
        b"tar payload\n"
    );
}

#[test]
fn tar_gzip_archive() {
    let dir = TestDir::new();
    fs::write(dir.path().join("one.txt"), b"1\n").unwrap();
    fs::write(dir.path().join("two.txt"), b"2\n").unwrap();
    let archive = dir.path().join("bundle.tar.gz");
    let extract = dir.path().join("extract");
    fs::create_dir(&extract).unwrap();

    assert_eq!(
        Rustbox::new()
            .current_dir(dir.path())
            .applet("tar")
            .args([
                "-czf",
                archive.file_name().unwrap().to_str().unwrap(),
                "one.txt",
                "two.txt"
            ])
            .status(),
        0
    );

    assert_eq!(
        Rustbox::new()
            .current_dir(&extract)
            .applet("tar")
            .args(["-xzf", archive.to_str().unwrap()])
            .status(),
        0
    );
    assert_eq!(fs::read(extract.join("one.txt")).unwrap(), b"1\n");
    assert_eq!(fs::read(extract.join("two.txt")).unwrap(), b"2\n");
}

#[test]
fn logrotate_keeps_total_size_small() {
    let dir = TestDir::new();
    let conf = dir.path().join("logrotate.conf");
    let log = dir.path().join("app.log");
    fs::write(
        &conf,
        format!(
            "{} {{
    rotate 3
    compress
    totalsize 1536k
    missingok
}}",
            log.display()
        ),
    )
    .unwrap();

    fs::write(&log, vec![b'x'; 600_000]).unwrap();
    assert_eq!(
        Rustbox::new()
            .applet("logrotate")
            .args(["-f", conf.to_str().unwrap()])
            .status(),
        0
    );

    fs::write(&log, vec![b'y'; 600_000]).unwrap();
    assert_eq!(
        Rustbox::new()
            .applet("logrotate")
            .args(["-f", conf.to_str().unwrap()])
            .status(),
        0
    );

    fs::write(&log, vec![b'z'; 600_000]).unwrap();
    assert_eq!(
        Rustbox::new()
            .applet("logrotate")
            .args(["-f", conf.to_str().unwrap()])
            .status(),
        0
    );

    let total: u64 = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    assert!(
        total < 2 * 1024 * 1024,
        "rotated logs should stay under 2 MiB, got {total} bytes"
    );
    assert!(log.exists());
    assert!(dir.path().join("app.log.1.gz").exists());
}

#[test]
fn tar_utf8_in_bundled_options_does_not_panic() {
    let status = Rustbox::new().applet("tar").args(["-cccɓcccscc2"]).status();
    assert_ne!(status, 0);
}
