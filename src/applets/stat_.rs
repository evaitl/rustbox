use crate::sys;
use crate::{eprintln, usage};
use rustix::fs::FileType;

pub fn run(args: &[&str]) -> i32 {
    let mut format: Option<String> = None;
    let mut terse = false;
    let mut follow = false;
    let mut quiet = false;
    let mut paths: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-L" => follow = true,
            "-t" => terse = true,
            "-f" => quiet = true,
            "-c" => {
                i += 1;
                if i >= args.len() {
                    usage("stat", "option requires an argument -- 'c'");
                    return 1;
                }
                format = Some(args[i].to_string());
            }
            s if s.starts_with("-c") && s.len() > 2 => {
                format = Some(s[2..].to_string());
            }
            s if s.starts_with('-') => {
                usage("stat", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
        i += 1;
    }

    if paths.is_empty() {
        usage("stat", "missing operand");
        return 1;
    }

    let mut status = 0;
    for path in paths {
        match stat_path(path, follow, terse, format.as_deref()) {
            Ok(()) => {}
            Err(e) => {
                if !quiet {
                    eprintln(format!("stat: cannot stat '{path}': {e}"));
                }
                status = 1;
            }
        }
    }
    status
}

fn stat_path(path: &str, follow: bool, terse: bool, format: Option<&str>) -> sys::Result<()> {
    let st = if follow {
        sys::stat(path)?
    } else {
        sys::lstat(path)?
    };

    if let Some(spec) = format {
        print!("{}", format_stat(&st, path, spec));
        return Ok(());
    }

    if terse {
        println!("{}", file_type_name(&st));
        return Ok(());
    }

    let mode = st.st_mode & 0o7777;
    println!("  File: {path}");
    println!(
        "  Size: {}    Blocks: {}",
        st.st_size,
        (st.st_size + 511) / 512
    );
    println!("  Mode: ({mode:04o}/{})", sys::mode_string(&st));
    println!("  Uid: {:5}   Gid: {:5}", st.st_uid, st.st_gid);
    println!("  Modify: {}", st.st_mtime.max(0));
    Ok(())
}

fn file_type_name(st: &rustix::fs::Stat) -> &'static str {
    let ft = FileType::from_raw_mode(st.st_mode);
    if ft.is_dir() {
        "directory"
    } else if ft.is_symlink() {
        "symbolic link"
    } else if ft.is_file() {
        "regular file"
    } else if ft.is_block_device() {
        "block special file"
    } else if ft.is_char_device() {
        "character special file"
    } else if ft.is_fifo() {
        "fifo"
    } else if ft.is_socket() {
        "socket"
    } else {
        "unknown"
    }
}

fn format_stat(st: &rustix::fs::Stat, path: &str, spec: &str) -> String {
    let mut out = String::new();
    let mut chars = spec.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.next() {
                Some('a') => {
                    let mode = st.st_mode & 0o7777;
                    out.push_str(&format!("{mode:03o}"));
                }
                Some('n') => out.push_str(path),
                Some('s') => out.push_str(&st.st_size.to_string()),
                Some('F') => out.push_str(file_type_name(st)),
                Some('u') => out.push_str(&st.st_uid.to_string()),
                Some('g') => out.push_str(&st.st_gid.to_string()),
                Some('%') => out.push('%'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(ch);
        }
    }
    out
}
