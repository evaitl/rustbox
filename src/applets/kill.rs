use crate::sys;
use crate::{eprintln, usage};
use rustix::process::Signal;

pub fn run(args: &[&str]) -> i32 {
    if args.first().copied() == Some("-l") {
        list_signals();
        return 0;
    }

    let mut test_only = false;
    let mut signal = Signal::TERM;
    let mut pids: Vec<u32> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = args[i];
        if arg == "-s" {
            i += 1;
            if i >= args.len() {
                usage("kill", "option requires an argument -- 's'");
                return 1;
            }
            match parse_signal(args[i]) {
                Some(sig) => signal = sig,
                None => {
                    usage("kill", "invalid signal");
                    return 1;
                }
            }
        } else if let Some(name) = arg.strip_prefix("-SIG") {
            match parse_signal(name) {
                Some(sig) => signal = sig,
                None => {
                    usage("kill", "invalid signal");
                    return 1;
                }
            }
        } else if let Some(num) = arg.strip_prefix('-') {
            if num == "0" {
                test_only = true;
            } else if num.chars().all(|c| c.is_ascii_digit()) {
                match parse_signal(num) {
                    Some(sig) => signal = sig,
                    None => {
                        usage("kill", "invalid signal");
                        return 1;
                    }
                }
            } else {
                usage("kill", &format!("invalid option -- '{arg}'"));
                return 1;
            }
        } else if let Ok(pid) = arg.parse::<u32>() {
            pids.push(pid);
        } else {
            usage("kill", &format!("invalid argument -- '{arg}'"));
            return 1;
        }
        i += 1;
    }

    if pids.is_empty() {
        usage("kill", "usage: kill [-s SIG] PID...");
        return 1;
    }

    let mut status = 0;
    for pid in pids {
        let result = if test_only {
            sys::test_kill_pid(pid)
        } else {
            sys::kill_pid(pid, signal)
        };
        if let Err(e) = result {
            eprintln(format!("kill: ({pid}): {e}"));
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
        "ILL" => Some(Signal::ILL),
        "TRAP" => Some(Signal::TRAP),
        "ABRT" | "IOT" => Some(Signal::ABORT),
        "BUS" => Some(Signal::BUS),
        "FPE" => Some(Signal::FPE),
        "KILL" => Some(Signal::KILL),
        "USR1" => Some(Signal::USR1),
        "SEGV" => Some(Signal::SEGV),
        "USR2" => Some(Signal::USR2),
        "PIPE" => Some(Signal::PIPE),
        "ALRM" => Some(Signal::ALARM),
        "TERM" => Some(Signal::TERM),
        "CHLD" => Some(Signal::CHILD),
        "CONT" => Some(Signal::CONT),
        "STOP" => Some(Signal::STOP),
        "TSTP" => Some(Signal::TSTP),
        "TTIN" => Some(Signal::TTIN),
        "TTOU" => Some(Signal::TTOU),
        _ => None,
    }
}

fn list_signals() {
    const SIGNALS: &[(&str, i32)] = &[
        ("HUP", libc::SIGHUP),
        ("INT", libc::SIGINT),
        ("QUIT", libc::SIGQUIT),
        ("ILL", libc::SIGILL),
        ("TRAP", libc::SIGTRAP),
        ("ABRT", libc::SIGABRT),
        ("BUS", libc::SIGBUS),
        ("FPE", libc::SIGFPE),
        ("KILL", libc::SIGKILL),
        ("USR1", libc::SIGUSR1),
        ("SEGV", libc::SIGSEGV),
        ("USR2", libc::SIGUSR2),
        ("PIPE", libc::SIGPIPE),
        ("ALRM", libc::SIGALRM),
        ("TERM", libc::SIGTERM),
        ("CHLD", libc::SIGCHLD),
        ("CONT", libc::SIGCONT),
        ("STOP", libc::SIGSTOP),
        ("TSTP", libc::SIGTSTP),
        ("TTIN", libc::SIGTTIN),
        ("TTOU", libc::SIGTTOU),
    ];
    for (name, num) in SIGNALS {
        println!("{num} {name}");
    }
}
