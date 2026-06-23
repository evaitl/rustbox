use crate::sys;
use crate::{eprintln, usage};
use rustix::fs::FileType;

pub fn run(args: &[&str]) -> i32 {
    let mut canonical = false;
    let mut no_newline = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-f" | "--canonicalize" => canonical = true,
            "-n" | "--no-suffix" => no_newline = true,
            s if s.starts_with('-') => {
                usage("readlink", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if paths.is_empty() {
        usage("readlink", "missing operand");
        return 1;
    }

    for (idx, path) in paths.iter().enumerate() {
        let result = if canonical {
            canonicalize(path)
        } else {
            sys::read_link(path)
        };
        match result {
            Ok(value) => {
                if no_newline {
                    print!("{value}");
                } else {
                    println!("{value}");
                }
            }
            Err(e) => {
                eprintln(format!("readlink: {path}: {e}"));
                return 1;
            }
        }
        if no_newline && idx + 1 < paths.len() {
            print!(" ");
        }
    }
    0
}

fn canonicalize(path: &str) -> sys::Result<String> {
    let abs = make_absolute(path)?;
    let resolved = resolve_symlinks(&abs)?;
    sys::stat(&resolved)?;
    Ok(resolved)
}

fn make_absolute(path: &str) -> sys::Result<String> {
    if path.starts_with('/') {
        return Ok(clean_path(path));
    }
    let cwd = sys::current_dir()?;
    if cwd == "/" {
        Ok(clean_path(&format!("/{path}")))
    } else {
        Ok(clean_path(&format!("{cwd}/{path}")))
    }
}

fn clean_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

fn resolve_symlinks(path: &str) -> sys::Result<String> {
    if path == "/" {
        return Ok("/".to_string());
    }
    let mut parts: Vec<String> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    let mut i = 0;
    while i < parts.len() {
        let current = format!("/{}", parts[..=i].join("/"));
        let ft = FileType::from_raw_mode(sys::lstat(&current)?.st_mode);
        if ft.is_symlink() {
            let target = sys::read_link(&current)?;
            let resolved = if target.starts_with('/') {
                clean_path(&target)
            } else {
                let parent = if i == 0 {
                    "/".to_string()
                } else {
                    format!("/{}", parts[..i].join("/"))
                };
                if parent == "/" {
                    clean_path(&format!("/{target}"))
                } else {
                    clean_path(&format!("{parent}/{target}"))
                }
            };
            let resolved_parts: Vec<String> = resolved
                .trim_start_matches('/')
                .split('/')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect();
            if target.starts_with('/') {
                parts = resolved_parts;
                i = 0;
            } else {
                parts.truncate(i);
                parts.extend(resolved_parts.into_iter().skip(i));
            }
            continue;
        }
        i += 1;
    }
    Ok(if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    })
}
