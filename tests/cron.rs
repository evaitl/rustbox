mod support;

use std::fs;
use support::{Rustbox, TestDir};

#[test]
fn cron_dry_run_validates_crontab() {
    let dir = TestDir::new();
    let cron_dir = dir.join("crontabs");
    fs::create_dir(&cron_dir).unwrap();
    fs::write(cron_dir.join("root"), "* * * * * /bin/true\n").unwrap();
    assert_eq!(
        Rustbox::new()
            .applet("cron")
            .args(["-n", "-c", cron_dir.to_str().unwrap()])
            .status(),
        0
    );
}

#[test]
fn cron_dry_run_rejects_invalid_crontab() {
    let dir = TestDir::new();
    let cron_dir = dir.join("crontabs");
    fs::create_dir(&cron_dir).unwrap();
    fs::write(cron_dir.join("root"), "not a cron line\n").unwrap();
    assert_eq!(
        Rustbox::new()
            .applet("cron")
            .args(["-n", "-c", cron_dir.to_str().unwrap()])
            .status(),
        1
    );
}
