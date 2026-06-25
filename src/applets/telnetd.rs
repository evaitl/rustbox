use crate::net::telnetd::{self, Config};
use crate::passwd_auth;
use crate::sys;
use crate::{eprintln, usage};
use std::path::Path;

pub fn run(args: &[&str]) -> i32 {
    let mut foreground = false;
    let mut config_path = telnetd::DEFAULT_CONFIG.to_string();
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
                    usage("telnetd", "option requires an argument -- 'c'");
                    return 1;
                }
                config_path = args[i].to_string();
            }
            "-l" => {
                i += 1;
                if i >= args.len() {
                    usage("telnetd", "option requires an argument -- 'l'");
                    return 1;
                }
                listen_override = Some(args[i].to_string());
            }
            "-p" => {
                i += 1;
                if i >= args.len() {
                    usage("telnetd", "option requires an argument -- 'p'");
                    return 1;
                }
                port_override = args[i].parse().ok();
                if port_override.is_none() {
                    usage("telnetd", "invalid port");
                    return 1;
                }
            }
            "-P" => {
                i += 1;
                if i >= args.len() {
                    usage("telnetd", "option requires an argument -- 'P'");
                    return 1;
                }
                passwd_override = Some(args[i].to_string());
            }
            "-h" | "--help" => {
                usage(
                    "telnetd",
                    "usage: telnetd [-f] [-c CONFIG] [-l ADDR] [-p PORT] [-P PASSWD]",
                );
                return 0;
            }
            s if s.starts_with('-') => {
                usage("telnetd", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("telnetd", &format!("unexpected argument -- '{s}'"));
                return 1;
            }
        }
        i += 1;
    }

    let mut cfg = if Path::new(&config_path).exists() {
        match telnetd::load_config(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln(format!("telnetd: {config_path}: {e}"));
                return 1;
            }
        }
    } else if config_path == telnetd::DEFAULT_CONFIG {
        Config::default()
    } else {
        eprintln(format!("telnetd: {config_path}: not found"));
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

    let passwd = passwd_auth::load_auth_passwd(&cfg.passwd_path);
    if passwd.is_empty() {
        eprintln(format!(
            "telnetd: no bcrypt credentials in {} (see SECURITY.md)",
            cfg.passwd_path
        ));
        return 1;
    }

    if !foreground {
        if let Err(e) = sys::daemonize() {
            eprintln(format!("telnetd: {e}"));
            return 1;
        }
    }

    eprintln(format!(
        "telnetd: listening on {}:{} (plaintext; see SECURITY.md)",
        cfg.listen_addr, cfg.port
    ));

    match telnetd::serve(cfg, passwd) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("telnetd: {e}"));
            1
        }
    }
}
