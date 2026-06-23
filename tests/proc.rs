mod support;

use support::Rustbox;

#[test]
fn date_prints_default() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("date").stdout();
    assert!(!out.trim().is_empty());
}

#[test]
fn date_format_percent_y() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("date").arg("+%Y").stdout();
    assert_eq!(out.len(), 5); // e.g. "2026\n"
}

#[test]
fn ps_lists_init_or_shell() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("ps").stdout();
    assert!(out.contains("PID"));
    assert!(out.lines().count() > 1);
}

#[test]
fn kill_signal_zero_checks_child() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let comm = support::unique_comm("rbx-k0");
    let mut sleeper = support::spawn_sleep_with_comm(&comm, 30);
    let pid = sleeper.id().to_string();
    let status = Rustbox::new().applet("kill").args(["-0", &pid]).status();
    let _ = sleeper.child.kill();
    let _ = sleeper.child.wait();
    assert_eq!(status, 0);
}

#[test]
fn kill_lists_signals() {
    let out = Rustbox::new().applet("kill").arg("-l").stdout();
    assert!(out.contains("TERM"));
    assert!(out.contains("KILL"));
}

#[test]
fn uptime_prints_load_average() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("uptime").stdout();
    assert!(out.contains("load average:"));
    assert!(out.contains("up "));
}

#[test]
fn free_prints_mem_line() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("free").stdout();
    assert!(out.contains("Mem:"));
    assert!(out.contains("Swap:"));
}

#[test]
fn top_single_snapshot() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let out = Rustbox::new().applet("top").args(["-n", "1"]).stdout();
    assert!(out.contains("PID"));
    assert!(out.contains("RSS"));
}

#[test]
fn killall_no_match_fails() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let status = Rustbox::new()
        .applet("killall")
        .arg("rustbox-nonexistent-process-name")
        .status();
    assert_eq!(status, 1);
}

#[test]
fn killall_terminates_matching_process() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let comm = support::unique_comm("rbx-ka");
    let mut sleeper = support::spawn_sleep_with_comm(&comm, 60);
    let status = Rustbox::new().applet("killall").arg(&comm).status();
    let _ = sleeper.child.wait();
    assert_eq!(status, 0);
}
