mod support;

use support::Rustbox;

#[test]
fn sync_exits_zero() {
    assert_eq!(Rustbox::new().applet("sync").status(), 0);
}

#[test]
fn sysctl_reads_kernel_ostype() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new()
        .applet("sysctl")
        .arg("-n")
        .arg("kernel.ostype")
        .stdout();
    assert!(!out.trim().is_empty());
}

#[test]
fn sysctl_all_includes_kernel() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("sysctl").args(["-a"]).stdout();
    assert!(out.contains("kernel.ostype"));
}

#[test]
fn swapon_missing_operand_fails() {
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_ne!(Rustbox::new().applet("swapon").status(), 0);
}

#[test]
fn swapoff_missing_operand_fails() {
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_ne!(Rustbox::new().applet("swapoff").status(), 0);
}

#[test]
fn syslogd_help() {
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_eq!(Rustbox::new().applet("syslogd").arg("--help").status(), 0);
}
