use crate::net::dhcp::{apply_lease, dhcp_acquire};
use crate::{eprintln, usage};

pub(crate) struct Config {
    pub iface: String,
    pub tries: u32,
    pub quit: bool,
    pub fail_exit: bool,
    pub timeout_ms: u32,
}

pub(crate) fn parse_args(args: &[&str]) -> Result<Config, i32> {
    let mut iface = "eth0".to_string();
    let mut tries = 3u32;
    let mut quit = false;
    let mut fail_exit = false;
    let mut timeout_ms = 3000u32;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-i" => {
                i += 1;
                if i >= args.len() {
                    usage("udhcpc", "option requires an argument -- 'i'");
                    return Err(1);
                }
                iface = args[i].to_string();
            }
            "-t" => {
                i += 1;
                if i >= args.len() {
                    usage("udhcpc", "option requires an argument -- 't'");
                    return Err(1);
                }
                tries = args[i].parse().unwrap_or(3);
            }
            "-T" => {
                i += 1;
                if i >= args.len() {
                    usage("udhcpc", "option requires an argument -- 'T'");
                    return Err(1);
                }
                timeout_ms = args[i].parse::<u32>().unwrap_or(3).saturating_mul(1000);
            }
            "-q" => quit = true,
            "-n" => fail_exit = true,
            "-h" | "--help" => {
                usage(
                    "udhcpc",
                    "usage: udhcpc [-i IFACE] [-q] [-n] [-t N] [-T SEC]",
                );
                return Err(0);
            }
            s if s.starts_with('-') => {
                usage("udhcpc", &format!("invalid option -- '{s}'"));
                return Err(1);
            }
            s => {
                iface = s.to_string();
            }
        }
        i += 1;
    }

    Ok(Config {
        iface,
        tries,
        quit,
        fail_exit,
        timeout_ms,
    })
}

pub fn run(args: &[&str]) -> i32 {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(code) => return code,
    };

    match dhcp_acquire(&config.iface, config.tries, config.timeout_ms) {
        Ok(lease) => {
            if let Err(e) = apply_lease(&config.iface, &lease) {
                eprintln(format!("udhcpc: {e}"));
                return 1;
            }
            if !config.quit {
                println!(
                    "udhcpc: lease of {} obtained, lease time 86400",
                    crate::net::ipv4::format_ipv4(lease.ip)
                );
            }
            0
        }
        Err(e) => {
            eprintln(format!("udhcpc: {e}"));
            if config.fail_exit {
                1
            } else {
                0
            }
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
    let _ = parse_args(&refs);
}
