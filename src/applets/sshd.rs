use crate::net::sshd::{self, Config};
use crate::sys;
use crate::{eprintln, usage};
use std::path::Path;

pub fn run(args: &[&str]) -> i32 {
    let mut foreground = false;
    let mut config_path = sshd::DEFAULT_CONFIG.to_string();
    let mut listen_override: Option<String> = None;
    let mut port_override: Option<u16> = None;
    let mut passwd_override: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" | "-F" => foreground = true,
            "-c" => {
                i += 1;
                if i >= args.len() {
                    usage("sshd", "option requires an argument -- 'c'");
                    return 1;
                }
                config_path = args[i].to_string();
            }
            "-l" => {
                i += 1;
                if i >= args.len() {
                    usage("sshd", "option requires an argument -- 'l'");
                    return 1;
                }
                listen_override = Some(args[i].to_string());
            }
            "-p" => {
                i += 1;
                if i >= args.len() {
                    usage("sshd", "option requires an argument -- 'p'");
                    return 1;
                }
                port_override = args[i].parse().ok();
                if port_override.is_none() {
                    usage("sshd", "invalid port");
                    return 1;
                }
            }
            "-P" => {
                i += 1;
                if i >= args.len() {
                    usage("sshd", "option requires an argument -- 'P'");
                    return 1;
                }
                passwd_override = Some(args[i].to_string());
            }
            "-h" | "--help" => {
                usage(
                    "sshd",
                    "usage: sshd [-f] [-c CONFIG] [-l ADDR] [-p PORT] [-P PASSWD]",
                );
                return 0;
            }
            s if s.starts_with('-') => {
                usage("sshd", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("sshd", &format!("unexpected argument -- '{s}'"));
                return 1;
            }
        }
        i += 1;
    }

    let mut cfg = if Path::new(&config_path).exists() {
        match sshd::load_config(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln(format!("sshd: {config_path}: {e}"));
                return 1;
            }
        }
    } else if config_path == sshd::DEFAULT_CONFIG {
        Config::default()
    } else {
        eprintln(format!("sshd: {config_path}: not found"));
        return 1;
    };

    if let Some(addr) = listen_override {
        cfg.listen_addr = addr;
    }
    if let Some(port) = port_override {
        cfg.port = port;
    }
    if let Some(passwd) = passwd_override {
        cfg.passwd_path = passwd;
    }

    let passwd = sshd::load_passwd(&cfg.passwd_path);
    if passwd.is_empty() {
        eprintln(format!(
            "sshd: no bcrypt credentials in {} (see README)",
            cfg.passwd_path
        ));
        return 1;
    }

    if !foreground {
        if let Err(e) = sys::daemonize() {
            eprintln(format!("sshd: {e}"));
            return 1;
        }
    }

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln(format!("sshd: {e}"));
            return 1;
        }
    };

    match rt.block_on(sshd::serve(cfg)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("sshd: {e}"));
            1
        }
    }
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_parse_args(input: &str) {
    let args: Vec<String> = input
        .split_whitespace()
        .take(64)
        .map(String::from)
        .collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let mut i = 0;
    while i < refs.len() {
        match refs[i] {
            "-f" | "-F" => {}
            "-c" | "-l" | "-p" | "-P" => {
                i += 1;
            }
            "-h" | "--help" => return,
            s if s.starts_with('-') => return,
            _ => return,
        }
        i += 1;
    }
    // parsed sshd argv
}
