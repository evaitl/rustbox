use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

struct Options {
    null: bool,
    max_args: Option<usize>,
    no_run_if_empty: bool,
    command: Vec<String>,
}

pub fn run(args: &[&str]) -> i32 {
    let mut opts = Options {
        null: false,
        max_args: None,
        no_run_if_empty: false,
        command: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-0" => opts.null = true,
            "-r" => opts.no_run_if_empty = true,
            "-n" => {
                i += 1;
                if i >= args.len() {
                    usage("xargs", "option requires an argument -- 'n'");
                    return 1;
                }
                opts.max_args = match args[i].parse() {
                    Ok(n) if n > 0 => Some(n),
                    _ => {
                        usage("xargs", "invalid number of arguments");
                        return 1;
                    }
                };
            }
            s if s.starts_with('-') => {
                usage("xargs", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                opts.command.push(s.to_string());
                i += 1;
                while i < args.len() {
                    opts.command.push(args[i].to_string());
                    i += 1;
                }
                break;
            }
        }
        i += 1;
    }

    if opts.command.is_empty() {
        opts.command.push("echo".to_string());
    }

    let items = match read_items(stdio::stdin(), opts.null) {
        Ok(items) => items,
        Err(e) => {
            eprintln(format!("xargs: read error: {e}"));
            return 1;
        }
    };

    if items.is_empty() {
        return if opts.no_run_if_empty {
            0
        } else {
            run_batch(&opts.command, &[])
        };
    }

    let max_args = opts.max_args.unwrap_or(usize::MAX);
    let mut status = 0;
    let mut batch = Vec::new();
    for item in items {
        batch.push(item);
        if batch.len() >= max_args {
            let code = run_batch(&opts.command, &batch);
            if code != 0 {
                status = code;
            }
            batch.clear();
        }
    }
    if !batch.is_empty() {
        let code = run_batch(&opts.command, &batch);
        if code != 0 {
            status = code;
        }
    }
    status
}

fn read_items<Fd: rustix::fd::AsFd>(fd: Fd, null: bool) -> sys::Result<Vec<String>> {
    let bytes = sys::read_to_end(fd)?;
    if null {
        Ok(bytes
            .split(|&b| b == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect())
    } else {
        Ok(bytes
            .split(|&b| b == b'\n' || b == b' ' || b == b'\t')
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect())
    }
}

fn run_batch(command: &[String], extra: &[String]) -> i32 {
    let program = &command[0];
    let mut args: Vec<&str> = command[1..].iter().map(String::as_str).collect();
    for item in extra {
        args.push(item);
    }
    match sys::spawn_argv(program, &args) {
        Ok(pid) => sys::wait_pid(pid).unwrap_or(1),
        Err(e) => {
            eprintln(format!("xargs: {program}: {e}"));
            127
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits_whitespace_input() {
        let items: Vec<String> = b"one two\nthree"
            .split(|&b| b == b'\n' || b == b' ' || b == b'\t')
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect();
        assert_eq!(items, vec!["one", "two", "three"]);
    }
}
