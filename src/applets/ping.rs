use crate::net::ping;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut count = u32::MAX;
    let mut timeout = 3u32;
    let mut quiet = false;
    let mut host: Option<&str> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args[i];
        if arg == "-c" {
            i += 1;
            if i >= args.len() {
                usage("ping", "option requires an argument -- 'c'");
                return 1;
            }
            count = match args[i].parse() {
                Ok(n) if n > 0 => n,
                _ => {
                    usage("ping", "invalid count");
                    return 1;
                }
            };
        } else if arg == "-W" || arg == "-w" {
            i += 1;
            if i >= args.len() {
                usage("ping", "option requires an argument");
                return 1;
            }
            timeout = match args[i].parse() {
                Ok(n) if n > 0 => n,
                _ => {
                    usage("ping", "invalid timeout");
                    return 1;
                }
            };
        } else if arg == "-q" {
            quiet = true;
        } else if arg.starts_with('-') {
            usage("ping", &format!("invalid option -- '{arg}'"));
            return 1;
        } else {
            host = Some(arg);
        }
        i += 1;
    }

    let host = match host {
        Some(h) => h,
        None => {
            usage("ping", "missing host operand");
            return 1;
        }
    };

    if count == u32::MAX {
        count = 4;
    }

    match ping::ping_host(host, count, timeout, quiet) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("ping: {host}: {e}"));
            1
        }
    }
}
