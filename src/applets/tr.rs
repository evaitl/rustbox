use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

pub fn run(args: &[&str]) -> i32 {
    let mut delete = false;
    let mut squeeze = false;
    let mut sets: Vec<&str> = Vec::new();
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-d" => delete = true,
            "-s" => squeeze = true,
            s if s.starts_with('-') => {
                usage("tr", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                if sets.len() < 2 {
                    sets.push(s);
                } else {
                    paths.push(s);
                }
            }
        }
    }

    if sets.is_empty() {
        usage("tr", "missing operand");
        return 1;
    }

    let from = expand_set(sets[0]);
    let to = if sets.len() > 1 {
        expand_set(sets[1])
    } else {
        Vec::new()
    };

    if paths.is_empty() {
        return tr_fd(stdio::stdin(), &from, &to, delete, squeeze);
    }

    let mut status = 0;
    for path in paths {
        match sys::open_read(path) {
            Ok(fd) => {
                if tr_fd(fd, &from, &to, delete, squeeze) != 0 {
                    status = 1;
                }
            }
            Err(e) => {
                eprintln(format!("tr: {path}: {e}"));
                status = 1;
            }
        }
    }
    status
}

fn expand_set(spec: &str) -> Vec<char> {
    let mut out = Vec::new();
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 2 < chars.len() && chars[i + 1] == '-' {
            let start = chars[i];
            let end = chars[i + 2];
            if start <= end {
                for c in start..=end {
                    out.push(c);
                }
            }
            i += 3;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn tr_fd<Fd: rustix::fd::AsFd>(
    fd: Fd,
    from: &[char],
    to: &[char],
    delete: bool,
    squeeze: bool,
) -> i32 {
    let mut map = [0u8; 256];
    for (i, slot) in map.iter_mut().enumerate() {
        *slot = i as u8;
    }
    if delete {
        for &ch in from {
            map[ch as usize] = 0;
        }
    } else {
        for (idx, &ch) in from.iter().enumerate() {
            let mapped = to.get(idx).or_else(|| to.last()).copied().unwrap_or(ch);
            map[ch as usize] = mapped as u8;
        }
    }

    let result = sys::for_each_line(fd, |line| {
        let mut prev_space = false;
        for &byte in line {
            let mapped = map[byte as usize];
            if delete {
                if mapped == 0 {
                    continue;
                }
                print!("{}", mapped as char);
                continue;
            }
            let ch = mapped as char;
            if squeeze && ch.is_whitespace() {
                if prev_space {
                    continue;
                }
                prev_space = true;
                print!(" ");
            } else {
                prev_space = false;
                print!("{ch}");
            }
        }
        println!();
        true
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("tr: read error: {e}"));
            1
        }
    }
}
