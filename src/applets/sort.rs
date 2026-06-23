use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

pub fn run(args: &[&str]) -> i32 {
    let mut reverse = false;
    let mut unique = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-r" => reverse = true,
            "-u" => unique = true,
            s if s.starts_with('-') => {
                usage("sort", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    let mut lines = Vec::new();
    if paths.is_empty() {
        if let Err(code) = read_lines(stdio::stdin(), &mut lines) {
            return code;
        }
    } else {
        for path in paths {
            match sys::open_read(path) {
                Ok(fd) => {
                    if let Err(code) = read_lines(fd, &mut lines) {
                        eprintln(format!("sort: {path}: read error"));
                        return code;
                    }
                }
                Err(e) => {
                    eprintln(format!("sort: {path}: {e}"));
                    return 1;
                }
            }
        }
    }

    lines.sort();
    if reverse {
        lines.reverse();
    }

    let mut prev: Option<&str> = None;
    for line in &lines {
        if unique {
            if prev == Some(line.as_str()) {
                continue;
            }
            prev = Some(line.as_str());
        }
        println!("{line}");
    }
    0
}

fn read_lines<Fd: rustix::fd::AsFd>(fd: Fd, out: &mut Vec<String>) -> Result<(), i32> {
    sys::for_each_line(fd, |line| {
        let text = String::from_utf8_lossy(line);
        out.push(text.strip_suffix('\n').unwrap_or(&text).to_string());
        true
    })
    .map_err(|_| 1)
}
