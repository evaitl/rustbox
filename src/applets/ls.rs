use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut long = false;
    let mut all = false;
    let mut one_per_line = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-l" => long = true,
            "-a" | "-A" => all = true,
            "-1" => one_per_line = true,
            s if s.starts_with('-') && s.len() > 1 => {
                for c in s[1..].chars() {
                    match c {
                        'l' => long = true,
                        'a' | 'A' => all = true,
                        '1' => one_per_line = true,
                        _ => {
                            usage("ls", &format!("invalid option -- '{c}'"));
                            return 1;
                        }
                    }
                }
            }
            s => paths.push(s),
        }
    }

    if paths.is_empty() {
        paths.push(".");
    }

    let mut status = 0;
    let multi = paths.len() > 1;
    let mut need_newline = false;

    for path in paths {
        match list_path(path, long, all, one_per_line, multi, &mut need_newline) {
            Ok(()) => {}
            Err(e) => {
                eprintln(format!("ls: cannot access '{path}': {e}"));
                status = 1;
            }
        }
    }
    if need_newline {
        println!();
    }
    status
}

fn list_path(
    path: &str,
    long: bool,
    all: bool,
    one_per_line: bool,
    show_dir_name: bool,
    need_newline: &mut bool,
) -> sys::Result<()> {
    if sys::is_directory(path) {
        if show_dir_name {
            println!("{path}:");
        }
        for entry in sys::read_dir(path)? {
            if entry.name.starts_with('.') && !all {
                continue;
            }
            let child = join_path(path, &entry.name);
            print_entry(
                &child,
                &entry.name,
                entry.file_type.is_dir(),
                long,
                one_per_line,
                need_newline,
            )?;
        }
        if show_dir_name {
            println!();
        }
    } else {
        let name = file_name(path);
        print_entry(path, name, false, long, one_per_line, need_newline)?;
    }
    Ok(())
}

fn print_entry(
    path: &str,
    name: &str,
    is_dir_hint: bool,
    long: bool,
    one_per_line: bool,
    need_newline: &mut bool,
) -> sys::Result<()> {
    if long {
        let st = sys::stat(path)?;
        let mode = sys::mode_string(&st);
        let size = st.st_size;
        let mtime = sys::format_mtime(&st);
        let is_dir = rustix::fs::FileType::from_raw_mode(st.st_mode).is_dir();
        let suffix = if is_dir { "/" } else { "" };
        println!("{mode} {size:>8} {mtime} {name}{suffix}");
    } else if one_per_line {
        println!("{name}");
    } else {
        let suffix = if is_dir_hint { "/" } else { "" };
        print!("{name}{suffix}  ");
        *need_newline = true;
    }
    Ok(())
}

fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
