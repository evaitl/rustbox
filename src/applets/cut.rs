use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

pub fn run(args: &[&str]) -> i32 {
    let mut delim = b'\t';
    let mut fields: Vec<usize> = Vec::new();
    let mut paths: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == "-d" {
            i += 1;
            if i >= args.len() {
                usage("cut", "option requires an argument -- 'd'");
                return 1;
            }
            delim = parse_delim(args[i]);
        } else if let Some(ch) = arg.strip_prefix("-d") {
            if ch.is_empty() {
                usage("cut", "option requires an argument -- 'd'");
                return 1;
            }
            delim = parse_delim(ch);
        } else if arg == "-f" {
            i += 1;
            if i >= args.len() {
                usage("cut", "option requires an argument -- 'f'");
                return 1;
            }
            if parse_fields(args[i], &mut fields).is_err() {
                usage("cut", "invalid field list");
                return 1;
            }
        } else if let Some(list) = arg.strip_prefix("-f") {
            if list.is_empty() {
                usage("cut", "option requires an argument -- 'f'");
                return 1;
            }
            if parse_fields(list, &mut fields).is_err() {
                usage("cut", "invalid field list");
                return 1;
            }
        } else if arg.starts_with('-') {
            usage("cut", &format!("invalid option -- '{arg}'"));
            return 1;
        } else {
            paths.push(arg);
        }
        i += 1;
    }

    if fields.is_empty() {
        usage("cut", "you must specify -f");
        return 1;
    }

    if paths.is_empty() {
        return cut_fd(stdio::stdin(), delim, &fields);
    }

    let mut status = 0;
    for path in paths {
        match sys::open_read(path) {
            Ok(fd) => {
                if cut_fd(fd, delim, &fields) != 0 {
                    status = 1;
                }
            }
            Err(e) => {
                eprintln(format!("cut: {path}: {e}"));
                status = 1;
            }
        }
    }
    status
}

fn parse_delim(spec: &str) -> u8 {
    spec.chars().next().map(|c| c as u8).unwrap_or(b'\t')
}

fn parse_fields(spec: &str, out: &mut Vec<usize>) -> Result<(), ()> {
    for part in spec.split(',') {
        if part.is_empty() {
            return Err(());
        }
        out.push(part.parse().map_err(|_| ())?);
    }
    Ok(())
}

fn cut_fd<Fd: rustix::fd::AsFd>(fd: Fd, delim: u8, fields: &[usize]) -> i32 {
    let result = sys::for_each_line(fd, |line| {
        let text = String::from_utf8_lossy(line);
        let line = text.strip_suffix('\n').unwrap_or(&text);
        let parts: Vec<&str> = line.split(|c| c as u8 == delim).collect();
        let mut first = true;
        for &field in fields {
            if field == 0 || field > parts.len() {
                continue;
            }
            if !first {
                print!("{}", delim as char);
            }
            print!("{}", parts[field - 1]);
            first = false;
        }
        println!();
        true
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("cut: read error: {e}"));
            1
        }
    }
}
