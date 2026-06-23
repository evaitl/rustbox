use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

pub fn run(args: &[&str]) -> i32 {
    let mut lines = 10usize;
    let mut paths: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-n" => {
                i += 1;
                if i >= args.len() {
                    usage("head", "option requires an argument -- 'n'");
                    return 1;
                }
                lines = match args[i].parse() {
                    Ok(n) => n,
                    Err(_) => {
                        usage("head", "invalid number of lines");
                        return 1;
                    }
                };
            }
            s if s.starts_with("-n") => {
                lines = match s[2..].parse() {
                    Ok(n) => n,
                    Err(_) => {
                        usage("head", "invalid number of lines");
                        return 1;
                    }
                };
            }
            s if s.starts_with('-') => {
                usage("head", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
        i += 1;
    }

    if paths.is_empty() {
        return print_head(stdio::stdin(), lines);
    }

    for path in paths {
        match sys::open_read(path) {
            Ok(fd) => {
                if print_head(fd, lines) != 0 {
                    return 1;
                }
            }
            Err(e) => {
                eprintln(format!("head: {path}: {e}"));
                return 1;
            }
        }
    }
    0
}

fn print_head<Fd: rustix::fd::AsFd>(fd: Fd, n: usize) -> i32 {
    let mut count = 0usize;
    let result = sys::for_each_line(fd, |line| {
        if count >= n {
            return false;
        }
        let text = String::from_utf8_lossy(line);
        println!("{text}");
        count += 1;
        true
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("head: read error: {e}"));
            1
        }
    }
}
