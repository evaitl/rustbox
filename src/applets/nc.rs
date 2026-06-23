use crate::net::netcat::{self, Config};
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut listen = false;
    let mut udp = false;
    let mut port: Option<u16> = None;
    let mut host: Option<String> = None;
    let mut timeout_secs = 0u32;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-l" => listen = true,
            "-u" => udp = true,
            "-p" => {
                i += 1;
                if i >= args.len() {
                    usage("nc", "option requires an argument -- 'p'");
                    return 1;
                }
                port = match args[i].parse() {
                    Ok(p) if p > 0 => Some(p),
                    _ => {
                        usage("nc", "invalid port");
                        return 1;
                    }
                };
            }
            "-w" => {
                i += 1;
                if i >= args.len() {
                    usage("nc", "option requires an argument -- 'w'");
                    return 1;
                }
                timeout_secs = match args[i].parse() {
                    Ok(n) => n,
                    _ => {
                        usage("nc", "invalid timeout");
                        return 1;
                    }
                };
            }
            "-h" | "--help" => {
                usage(
                    "nc",
                    "usage: nc [-l] [-u] [-p port] [-w timeout] [HOST] [PORT]",
                );
                return 0;
            }
            s if s.starts_with('-') => {
                usage("nc", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                if host.is_none() {
                    host = Some(s.to_string());
                } else if port.is_none() {
                    port = s.parse().ok();
                    if port.is_none() {
                        usage("nc", "invalid port");
                        return 1;
                    }
                } else {
                    usage("nc", "too many arguments");
                    return 1;
                }
            }
        }
        i += 1;
    }

    let port = match port {
        Some(p) => p,
        None => {
            usage("nc", "missing port");
            return 1;
        }
    };

    if !listen && host.is_none() {
        usage("nc", "missing host");
        return 1;
    }

    let cfg = Config {
        listen,
        udp,
        port,
        host,
        timeout_secs,
    };

    match netcat::run(cfg) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("nc: {e}"));
            1
        }
    }
}
