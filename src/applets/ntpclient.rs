use crate::net::ntp;
use crate::{eprintln, usage};

const DEFAULT_SERVER: &str = "129.6.15.28"; // time.nist.gov

pub fn run(args: &[&str]) -> i32 {
    let mut set_time = false;
    let mut timeout = 5u32;
    let mut server = DEFAULT_SERVER.to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-s" | "-S" => set_time = true,
            "-t" | "-T" => {
                i += 1;
                if i >= args.len() {
                    usage("ntpclient", "option requires an argument");
                    return 1;
                }
                timeout = match args[i].parse() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        usage("ntpclient", "invalid timeout");
                        return 1;
                    }
                };
            }
            "-h" | "--help" => {
                usage("ntpclient", "usage: ntpclient [-s] [-t SEC] [SERVER]");
                return 0;
            }
            s if s.starts_with('-') => {
                usage("ntpclient", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => server = s.to_string(),
        }
        i += 1;
    }

    match ntp::query(&server, timeout) {
        Ok(result) => {
            println!(
                "ntpclient: server {} time {}.{:09}",
                server, result.unix_secs, result.unix_nsec
            );
            if set_time {
                match crate::sys::set_clock_realtime(result.unix_secs, result.unix_nsec) {
                    Ok(()) => {
                        println!("ntpclient: clock set");
                        0
                    }
                    Err(e) => {
                        eprintln(format!("ntpclient: settimeofday: {e}"));
                        1
                    }
                }
            } else {
                0
            }
        }
        Err(e) => {
            eprintln(format!("ntpclient: {server}: {e}"));
            1
        }
    }
}
