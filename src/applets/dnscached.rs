use crate::net::dnscached::{self, Config};
use crate::sys;
use crate::{eprintln, usage};
use std::path::Path;

pub fn run(args: &[&str]) -> i32 {
    let mut foreground = false;
    let mut config_path = dnscached::DEFAULT_CONFIG.to_string();
    let mut listen_override: Option<String> = None;
    let mut port_override: Option<u16> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" | "-F" => foreground = true,
            "-c" => {
                i += 1;
                if i >= args.len() {
                    usage("dnscached", "option requires an argument -- 'c'");
                    return 1;
                }
                config_path = args[i].to_string();
            }
            "-l" => {
                i += 1;
                if i >= args.len() {
                    usage("dnscached", "option requires an argument -- 'l'");
                    return 1;
                }
                listen_override = Some(args[i].to_string());
            }
            "-p" => {
                i += 1;
                if i >= args.len() {
                    usage("dnscached", "option requires an argument -- 'p'");
                    return 1;
                }
                port_override = args[i].parse().ok();
                if port_override.is_none() {
                    usage("dnscached", "invalid port");
                    return 1;
                }
            }
            "-h" | "--help" => {
                usage(
                    "dnscached",
                    "usage: dnscached [-f] [-c CONFIG] [-l ADDR] [-p PORT]",
                );
                return 0;
            }
            s if s.starts_with('-') => {
                usage("dnscached", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("dnscached", &format!("unexpected argument -- '{s}'"));
                return 1;
            }
        }
        i += 1;
    }

    let mut cfg = if Path::new(&config_path).exists() {
        match dnscached::load_config(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln(format!("dnscached: {config_path}: {e}"));
                return 1;
            }
        }
    } else if config_path == dnscached::DEFAULT_CONFIG {
        Config::default()
    } else {
        eprintln(format!("dnscached: {config_path}: not found"));
        return 1;
    };

    if let Some(addr) = listen_override {
        cfg.listen_addr = crate::net::ipv4::parse_ipv4(&addr).unwrap_or(cfg.listen_addr);
    }
    if let Some(port) = port_override {
        cfg.listen_port = port;
    }

    if !foreground {
        if let Err(e) = sys::daemonize() {
            eprintln(format!("dnscached: {e}"));
            return 1;
        }
    }

    match dnscached::serve(cfg) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("dnscached: {e}"));
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
            "-c" | "-l" | "-p" => {
                i += 1;
            }
            "-h" | "--help" => return,
            s if s.starts_with('-') => return,
            _ => return,
        }
        i += 1;
    }
    // parsed dnscached argv
}
