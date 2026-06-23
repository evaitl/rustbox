use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut priority: Option<i32> = None;
    let mut discard = false;
    let mut paths: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-p" => {
                i += 1;
                if i >= args.len() {
                    usage("swapon", "option requires an argument -- 'p'");
                    return 1;
                }
                priority = Some(match args[i].parse() {
                    Ok(p) => p,
                    Err(_) => {
                        usage("swapon", "invalid priority");
                        return 1;
                    }
                });
            }
            "-d" | "--discard" => discard = true,
            "-a" => {
                usage("swapon", "swapon -a not supported; specify a device");
                return 1;
            }
            s if s.starts_with('-') => {
                usage("swapon", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
        i += 1;
    }

    let _ = discard;

    if paths.is_empty() {
        usage("swapon", "usage: swapon [-p PRI] DEVICE");
        return 1;
    }

    let mut status = 0;
    for path in paths {
        if let Err(e) = sys::swapon(path, priority) {
            eprintln(format!("swapon: {path}: {e}"));
            status = 1;
        }
    }
    status
}
