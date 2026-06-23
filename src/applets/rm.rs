use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut recursive = false;
    let mut force = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-r" | "-R" | "--recursive" => recursive = true,
            "-f" | "--force" => force = true,
            s if s.starts_with('-') => {
                usage("rm", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if paths.is_empty() {
        usage("rm", "missing operand");
        return 1;
    }

    let mut status = 0;
    for path in paths {
        if !sys::exists(path) {
            if force {
                continue;
            }
            let err = rustix::io::Errno::NOENT;
            eprintln(format!("rm: cannot remove '{path}': {err}"));
            status = 1;
            continue;
        }
        if let Err(e) = sys::remove_path(path, recursive) {
            eprintln(format!("rm: cannot remove '{path}': {e}"));
            status = 1;
        }
    }
    status
}
