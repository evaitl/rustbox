use crate::sys;
use crate::{eprintln, usage};
use rustix::process::Signal;

pub fn run(args: &[&str]) -> i32 {
    let mut signal = Signal::TERM;
    let mut names: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = args[i];
        if arg == "-s" {
            i += 1;
            if i >= args.len() {
                usage("killall", "option requires an argument -- 's'");
                return 1;
            }
            match parse_signal(args[i]) {
                Some(sig) => signal = sig,
                None => {
                    usage("killall", "invalid signal");
                    return 1;
                }
            }
        } else if let Some(name) = arg.strip_prefix("-SIG") {
            match parse_signal(name) {
                Some(sig) => signal = sig,
                None => {
                    usage("killall", "invalid signal");
                    return 1;
                }
            }
        } else if let Some(num) = arg.strip_prefix('-') {
            if num.chars().all(|c| c.is_ascii_digit()) {
                match parse_signal(num) {
                    Some(sig) => signal = sig,
                    None => {
                        usage("killall", "invalid signal");
                        return 1;
                    }
                }
            } else {
                usage("killall", &format!("invalid option -- '{arg}'"));
                return 1;
            }
        } else if arg.starts_with('-') {
            usage("killall", &format!("invalid option -- '{arg}'"));
            return 1;
        } else {
            names.push(arg);
        }
        i += 1;
    }

    if names.is_empty() {
        usage("killall", "usage: killall [-s SIG] NAME...");
        return 1;
    }

    let procs = match sys::list_processes() {
        Ok(procs) => procs,
        Err(e) => {
            eprintln(format!("killall: {e}"));
            return 1;
        }
    };

    let mut status = 0;
    for name in names {
        let mut matched = false;
        for proc in &procs {
            if proc.comm == name {
                matched = true;
                if let Err(e) = sys::kill_pid(proc.pid, signal) {
                    eprintln(format!("killall: {}({}): {e}", name, proc.pid));
                    status = 1;
                }
            }
        }
        if !matched {
            eprintln(format!("killall: {name}: no process killed"));
            status = 1;
        }
    }
    status
}

fn parse_signal(name: &str) -> Option<Signal> {
    if let Ok(n) = name.parse::<i32>() {
        if n <= 0 {
            return None;
        }
        return Some(unsafe { Signal::from_raw_unchecked(n) });
    }
    let key = name
        .strip_prefix("SIG")
        .unwrap_or(name)
        .to_ascii_uppercase();
    match key.as_str() {
        "HUP" => Some(Signal::HUP),
        "INT" => Some(Signal::INT),
        "QUIT" => Some(Signal::QUIT),
        "KILL" => Some(Signal::KILL),
        "TERM" => Some(Signal::TERM),
        "USR1" => Some(Signal::USR1),
        "USR2" => Some(Signal::USR2),
        "CONT" => Some(Signal::CONT),
        "STOP" => Some(Signal::STOP),
        _ => None,
    }
}
