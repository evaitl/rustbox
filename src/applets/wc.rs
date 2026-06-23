use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

pub fn run(args: &[&str]) -> i32 {
    let mut show_lines = false;
    let mut show_words = false;
    let mut show_bytes = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-l" => show_lines = true,
            "-w" => show_words = true,
            "-c" | "-m" => show_bytes = true,
            s if s.starts_with('-') => {
                usage("wc", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if !show_lines && !show_words && !show_bytes {
        show_lines = true;
        show_words = true;
        show_bytes = true;
    }

    if paths.is_empty() {
        return count_fd(stdio::stdin(), "-", show_lines, show_words, show_bytes);
    }

    let mut total_lines = 0u64;
    let mut total_words = 0u64;
    let mut total_bytes = 0u64;
    let mut status = 0;
    let multi = paths.len() > 1;

    for path in &paths {
        match sys::open_read(path) {
            Ok(fd) => {
                let (lines, words, bytes) = count_fd_inner(fd, show_lines, show_words, show_bytes);
                print_counts(
                    lines, words, bytes, show_lines, show_words, show_bytes, path,
                );
                total_lines += lines;
                total_words += words;
                total_bytes += bytes;
            }
            Err(e) => {
                eprintln(format!("wc: {path}: {e}"));
                status = 1;
            }
        }
    }

    if multi && (show_lines || show_words || show_bytes) {
        print_counts(
            total_lines,
            total_words,
            total_bytes,
            show_lines,
            show_words,
            show_bytes,
            "total",
        );
    }

    status
}

fn count_fd<Fd: rustix::fd::AsFd>(
    fd: Fd,
    label: &str,
    show_lines: bool,
    show_words: bool,
    show_bytes: bool,
) -> i32 {
    let (lines, words, bytes) = count_fd_inner(fd, show_lines, show_words, show_bytes);
    print_counts(
        lines, words, bytes, show_lines, show_words, show_bytes, label,
    );
    0
}

fn count_fd_inner<Fd: rustix::fd::AsFd>(
    fd: Fd,
    show_lines: bool,
    show_words: bool,
    show_bytes: bool,
) -> (u64, u64, u64) {
    let mut lines = 0u64;
    let mut words = 0u64;
    let mut bytes = 0u64;

    let _ = sys::for_each_line(fd, |line| {
        if show_lines {
            lines += 1;
        }
        if show_words {
            let text = String::from_utf8_lossy(line);
            words += text.split_whitespace().count() as u64;
        }
        if show_bytes {
            bytes += line.len() as u64 + 1;
        }
        true
    });

    (lines, words, bytes)
}

fn print_counts(
    lines: u64,
    words: u64,
    bytes: u64,
    show_lines: bool,
    show_words: bool,
    show_bytes: bool,
    label: &str,
) {
    let mut parts = Vec::new();
    if show_lines {
        parts.push(format!("{lines:8}"));
    }
    if show_words {
        parts.push(format!("{words:8}"));
    }
    if show_bytes {
        parts.push(format!("{bytes:8}"));
    }
    println!("{} {}", parts.join(""), label);
}
