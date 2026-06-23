use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

struct Options {
    quiet: bool,
    scripts: Vec<String>,
    paths: Vec<String>,
}

pub fn run(args: &[&str]) -> i32 {
    let mut opts = Options {
        quiet: false,
        scripts: Vec::new(),
        paths: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-n" => opts.quiet = true,
            "-e" => {
                i += 1;
                if i >= args.len() {
                    usage("sed", "option requires an argument -- 'e'");
                    return 1;
                }
                opts.scripts.push(args[i].to_string());
            }
            s if s.starts_with('-') && s.len() > 1 => {
                usage("sed", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                if opts.scripts.is_empty() {
                    opts.scripts.push(s.to_string());
                } else {
                    opts.paths.push(s.to_string());
                }
            }
        }
        i += 1;
    }

    if opts.scripts.is_empty() {
        usage("sed", "missing script");
        return 1;
    }

    let program = match compile_program(&opts.scripts) {
        Ok(p) => p,
        Err(msg) => {
            eprintln(format!("sed: {msg}"));
            return 1;
        }
    };

    if opts.paths.is_empty() {
        return run_on_fd(stdio::stdin(), &program, opts.quiet);
    }

    let mut status = 0;
    for path in &opts.paths {
        match sys::open_read(path) {
            Ok(fd) => {
                if run_on_fd(fd, &program, opts.quiet) != 0 {
                    status = 1;
                }
            }
            Err(e) => {
                eprintln(format!("sed: {path}: {e}"));
                status = 1;
            }
        }
    }
    status
}

#[derive(Clone, Debug)]
struct Program {
    commands: Vec<Command>,
}

#[derive(Clone, Debug)]
struct Command {
    range: Option<Range>,
    action: Action,
}

#[derive(Clone, Copy, Debug)]
struct Range {
    start: u64,
    end: u64,
}

#[derive(Clone, Debug)]
enum Action {
    Substitute {
        pattern: String,
        replacement: String,
        global: bool,
    },
    Delete,
    Print,
    Quit,
}

fn compile_program(scripts: &[String]) -> Result<Program, &'static str> {
    let mut commands = Vec::new();
    for script in scripts {
        for part in script.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            commands.push(parse_command(part)?);
        }
    }
    Ok(Program { commands })
}

fn parse_command(part: &str) -> Result<Command, &'static str> {
    let (range, rest) = parse_address_prefix(part)?;
    if let Some(body) = rest.strip_prefix('s') {
        let (pattern, replacement, global) = parse_substitute(body)?;
        return Ok(Command {
            range,
            action: Action::Substitute {
                pattern,
                replacement,
                global,
            },
        });
    }
    let action = match rest {
        "d" => Action::Delete,
        "p" => Action::Print,
        "q" => Action::Quit,
        _ => return Err("unknown command"),
    };
    Ok(Command { range, action })
}

fn parse_address_prefix(part: &str) -> Result<(Option<Range>, &str), &'static str> {
    let (start, rest) = match parse_line_addr(part) {
        Some(v) => v,
        None => return Ok((None, part)),
    };
    if let Some(rest) = rest.strip_prefix(',') {
        let (end, rest) = parse_line_addr(rest).ok_or("invalid address")?;
        return Ok((Some(Range { start, end }), rest));
    }
    if rest.starts_with('s') || rest == "d" || rest == "p" || rest == "q" {
        return Ok((Some(Range { start, end: start }), rest));
    }
    Err("invalid address")
}

fn parse_line_addr(s: &str) -> Option<(u64, &str)> {
    if let Some(rest) = s.strip_prefix('$') {
        return Some((u64::MAX, rest));
    }
    let digits = s
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    let n = digits.parse().ok()?;
    let rest = &s[digits.len()..];
    Some((n, rest))
}

fn parse_substitute(rest: &str) -> Result<(String, String, bool), &'static str> {
    let delim = rest.chars().next().ok_or("invalid substitute command")?;
    let body = &rest[delim.len_utf8()..];
    let (pattern, tail) = split_delim(body, delim).ok_or("invalid substitute command")?;
    let (replacement, flags) = split_delim(tail, delim).ok_or("invalid substitute command")?;
    let global = flags.contains('g');
    Ok((pattern.to_string(), replacement.to_string(), global))
}

fn split_delim(s: &str, delim: char) -> Option<(&str, &str)> {
    let mut escaped = false;
    for (idx, ch) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == delim {
            return Some((&s[..idx], &s[idx + ch.len_utf8()..]));
        }
    }
    None
}

fn run_on_fd<Fd: rustix::fd::AsFd>(fd: Fd, program: &Program, quiet: bool) -> i32 {
    let lines = match read_lines(fd) {
        Ok(lines) => lines,
        Err(e) => {
            eprintln(format!("sed: read error: {e}"));
            return 1;
        }
    };
    let total_lines = lines.len() as u64;

    for (idx, line) in lines.into_iter().enumerate() {
        let line_no = (idx as u64) + 1;
        let mut out = line;
        let mut delete = false;
        let mut print_extra = false;
        let mut quit = false;

        for cmd in &program.commands {
            if !addr_matches(cmd.range, line_no, total_lines) {
                continue;
            }
            match &cmd.action {
                Action::Substitute {
                    pattern,
                    replacement,
                    global,
                } => {
                    out = substitute(&out, pattern, replacement, *global);
                }
                Action::Delete => delete = true,
                Action::Print => print_extra = true,
                Action::Quit => quit = true,
            }
            if quit {
                break;
            }
        }

        if delete {
            if quit {
                break;
            }
            continue;
        }

        if !quiet || print_extra {
            println!("{out}");
        }
        if quit {
            break;
        }
    }
    0
}

fn read_lines<Fd: rustix::fd::AsFd>(fd: Fd) -> sys::Result<Vec<String>> {
    let mut lines = Vec::new();
    sys::for_each_line(fd, |line| {
        let text = String::from_utf8_lossy(line);
        lines.push(text.strip_suffix('\n').unwrap_or(&text).to_string());
        true
    })?;
    Ok(lines)
}

fn addr_matches(range: Option<Range>, line_no: u64, total: u64) -> bool {
    let Some(range) = range else {
        return true;
    };
    let start = if range.start == u64::MAX {
        total.max(1)
    } else {
        range.start
    };
    let end = if range.end == u64::MAX {
        total.max(1)
    } else {
        range.end
    };
    line_no >= start && line_no <= end
}

fn substitute(line: &str, pattern: &str, replacement: &str, global: bool) -> String {
    if pattern.is_empty() {
        return line.to_string();
    }
    let mut out = String::new();
    let mut rest = line;
    loop {
        if let Some(idx) = rest.find(pattern) {
            out.push_str(&rest[..idx]);
            out.push_str(&expand_replacement(replacement, pattern));
            rest = &rest[idx + pattern.len()..];
            if !global {
                out.push_str(rest);
                break;
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

fn expand_replacement(replacement: &str, matched: &str) -> String {
    let mut out = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            out.push_str(matched);
        } else if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_once() {
        assert_eq!(substitute("hello", "l", "L", false), "heLlo");
    }

    #[test]
    fn substitutes_global() {
        assert_eq!(substitute("hello", "l", "L", true), "heLLo");
    }

    #[test]
    fn parses_delete_range() {
        let cmd = parse_command("2,3d").unwrap();
        assert!(matches!(cmd.action, Action::Delete));
        assert_eq!(cmd.range.unwrap().start, 2);
    }
}
