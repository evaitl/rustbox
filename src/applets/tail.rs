use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;
use std::collections::VecDeque;

pub fn run(args: &[&str]) -> i32 {
    let mut lines = 10usize;
    let mut paths: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-n" => {
                i += 1;
                if i >= args.len() {
                    usage("tail", "option requires an argument -- 'n'");
                    return 1;
                }
                lines = match args[i].parse() {
                    Ok(n) => n,
                    Err(_) => {
                        usage("tail", "invalid number of lines");
                        return 1;
                    }
                };
            }
            s if s.starts_with("-n") => {
                lines = match s[2..].parse() {
                    Ok(n) => n,
                    Err(_) => {
                        usage("tail", "invalid number of lines");
                        return 1;
                    }
                };
            }
            s if s.starts_with('-') => {
                usage("tail", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
        i += 1;
    }

    if paths.is_empty() {
        return print_tail(stdio::stdin(), lines);
    }

    for path in paths {
        match sys::open_read(path) {
            Ok(fd) => {
                if print_tail(fd, lines) != 0 {
                    return 1;
                }
            }
            Err(e) => {
                eprintln(format!("tail: {path}: {e}"));
                return 1;
            }
        }
    }
    0
}

fn print_tail<Fd: rustix::fd::AsFd>(fd: Fd, n: usize) -> i32 {
    let mut buf: VecDeque<String> = VecDeque::with_capacity(n);
    let result = sys::for_each_line(fd, |line| {
        let text = String::from_utf8_lossy(line).into_owned();
        if buf.len() == n {
            buf.pop_front();
        }
        buf.push_back(text);
        true
    });
    if let Err(e) = result {
        eprintln(format!("tail: read error: {e}"));
        return 1;
    }
    for line in buf {
        println!("{line}");
    }
    0
}
