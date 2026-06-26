use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

/// (applet name, Rust module name, `rustc-cfg` flag suffix)
const KNOWN_APPLETS: &[(&str, &str, &str)] = &[
    ("basename", "basename", "basename"),
    ("cat", "cat", "cat"),
    ("chmod", "chmod", "chmod"),
    ("chown", "chown", "chown"),
    ("cp", "cp", "cp"),
    ("cron", "cron", "cron"),
    ("cut", "cut", "cut"),
    ("dd", "dd", "dd"),
    ("date", "date", "date"),
    ("dig", "dig", "dig"),
    ("dirname", "dirname", "dirname"),
    ("dnscached", "dnscached", "dnscached"),
    ("dmesg", "dmesg", "dmesg"),
    ("echo", "echo", "echo"),
    ("env", "env", "env"),
    ("false", "false_", "false"),
    ("find", "find", "find"),
    ("free", "free", "free"),
    ("grep", "grep", "grep"),
    ("gzip", "gzip", "gzip"),
    ("head", "head", "head"),
    ("hostname", "hostname", "hostname"),
    ("ifconfig", "ifconfig", "ifconfig"),
    ("init", "init", "init"),
    ("kill", "kill", "kill"),
    ("killall", "killall", "killall"),
    ("ln", "ln", "ln"),
    ("logger", "logger", "logger"),
    ("logrotate", "logrotate", "logrotate"),
    ("ls", "ls", "ls"),
    ("mkdir", "mkdir", "mkdir"),
    ("mknod", "mknod", "mknod"),
    ("halt", "halt", "halt"),
    ("mount", "mount", "mount"),
    ("mdev", "mdev", "mdev"),
    ("mv", "mv", "mv"),
    ("nc", "nc", "nc"),
    ("ntpclient", "ntpclient", "ntpclient"),
    ("ping", "ping", "ping"),
    ("reboot", "reboot", "reboot"),
    ("pivot_root", "pivot_root", "pivot_root"),
    ("printenv", "printenv", "printenv"),
    ("printf", "printf", "printf"),
    ("ps", "ps", "ps"),
    ("pwd", "pwd", "pwd"),
    ("passwd", "passwd", "passwd"),
    ("readlink", "readlink", "readlink"),
    ("rm", "rm", "rm"),
    ("rmdir", "rmdir", "rmdir"),
    ("route", "route", "route"),
    ("sed", "sed", "sed"),
    ("rash", "sh", "sh"),
    ("sh", "sh", "sh"),
    ("sleep", "sleep", "sleep"),
    ("sshd", "sshd", "sshd"),
    ("telnetd", "telnetd", "telnetd"),
    ("su", "su", "su"),
    ("sort", "sort", "sort"),
    ("stat", "stat_", "stat"),
    ("switch_root", "switch_root", "switch_root"),
    ("swapoff", "swapoff", "swapoff"),
    ("swapon", "swapon", "swapon"),
    ("sync", "sync", "sync"),
    ("sysctl", "sysctl", "sysctl"),
    ("syslogd", "syslogd", "syslogd"),
    ("tail", "tail", "tail"),
    ("tar", "tar", "tar"),
    ("top", "top", "top"),
    ("tr", "tr", "tr"),
    ("thttpd", "thttpd", "thttpd"),
    ("udhcpc", "udhcpc", "udhcpc"),
    ("test", "test_", "test"),
    ("[", "test_", "test"),
    ("true", "true_", "true"),
    ("umount", "umount", "umount"),
    ("uptime", "uptime", "uptime"),
    ("uname", "uname", "uname"),
    ("vi", "vi", "vi"),
    ("wc", "wc", "wc"),
    ("wget", "wget", "wget"),
    ("xargs", "xargs", "xargs"),
];

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config_path = env::var("RUSTBOX_APPLETS_CONFIG")
        .map(|p| Path::new(&p).to_path_buf())
        .unwrap_or_else(|_| manifest_dir.join("applets.json"));

    println!("cargo:rerun-if-changed={}", config_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTBOX_APPLETS_CONFIG");

    let config_text = fs::read_to_string(&config_path).unwrap_or_else(|e| {
        panic!(
            "failed to read applet config {}: {e}",
            config_path.display()
        );
    });

    let config: AppletConfig = serde_json::from_str(&config_text).unwrap_or_else(|e| {
        panic!(
            "failed to parse applet config {}: {e}",
            config_path.display()
        );
    });

    let mut enabled = BTreeMap::new();
    for (name, module, cfg) in KNOWN_APPLETS {
        println!("cargo::rustc-check-cfg=cfg(applet_{cfg})");
        let on = config.applets.get(*name).copied().unwrap_or(false);
        if on {
            if *name == "dig" && env::var("CARGO_FEATURE_APPLET_DIG").is_err() {
                panic!(
                    "dig is enabled in {} but Cargo feature 'applet-dig' is disabled; \
                     enable it in Cargo.toml or pass --features applet-dig",
                    config_path.display()
                );
            }
            if *name == "dnscached" && env::var("CARGO_FEATURE_APPLET_DNSCACHED").is_err() {
                panic!(
                    "dnscached is enabled in {} but Cargo feature 'applet-dnscached' is disabled; \
                     enable it in Cargo.toml or pass --features applet-dnscached",
                    config_path.display()
                );
            }
            if *name == "telnetd" && env::var("CARGO_FEATURE_APPLET_PASSWD").is_err() {
                panic!(
                    "telnetd is enabled in {} but Cargo feature 'applet-passwd' is disabled; \
                     enable it in Cargo.toml or pass --features applet-passwd",
                    config_path.display()
                );
            }
            if *name == "sshd" && env::var("CARGO_FEATURE_APPLET_SSHD").is_err() {
                panic!(
                    "sshd is enabled in {} but Cargo feature 'applet-sshd' is disabled; \
                     enable it in Cargo.toml or pass --features applet-sshd",
                    config_path.display()
                );
            }
            if *name == "passwd" && env::var("CARGO_FEATURE_APPLET_PASSWD").is_err() {
                panic!(
                    "passwd is enabled in {} but Cargo feature 'applet-passwd' is disabled; \
                     enable it in Cargo.toml or pass --features applet-passwd",
                    config_path.display()
                );
            }
            enabled.insert(*name, (*module, *cfg));
            println!("cargo:rustc-cfg=applet_{cfg}");
        }
    }

    for key in config.applets.keys() {
        if !KNOWN_APPLETS.iter().any(|(name, _, _)| name == key) {
            panic!("applets.json contains unknown applet '{key}'");
        }
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let mut table = String::from("// @generated by build.rs from applets.json\n\n");
    table.push_str("pub const APPLETS: &[crate::Applet] = &[\n");
    for (name, (module, _)) in &enabled {
        table.push_str(&format!(
            "    crate::Applet {{ name: \"{name}\", run: crate::applets::{module}::run }},\n"
        ));
    }
    table.push_str("];\n");

    let out_path = Path::new(&out_dir).join("applets_table.rs");
    fs::write(&out_path, table).expect("failed to write applets_table.rs");
}

#[derive(serde::Deserialize)]
struct AppletConfig {
    applets: BTreeMap<String, bool>,
}
