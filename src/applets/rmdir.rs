use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut parents = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-p" | "--parents" => parents = true,
            s if s.starts_with('-') => {
                usage("rmdir", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if paths.is_empty() {
        usage("rmdir", "missing operand");
        return 1;
    }

    let mut status = 0;
    for path in paths {
        let result = if parents {
            remove_parents(path)
        } else {
            sys::remove_dir(path)
        };
        if let Err(e) = result {
            eprintln(format!("rmdir: failed to remove '{path}': {e}"));
            status = 1;
        }
    }
    status
}

fn remove_parents(path: &str) -> sys::Result<()> {
    let mut parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    while !parts.is_empty() {
        let sub = if path.starts_with('/') {
            format!("/{}", parts.join("/"))
        } else {
            parts.join("/")
        };
        sys::remove_dir(&sub)?;
        parts.pop();
    }
    Ok(())
}
