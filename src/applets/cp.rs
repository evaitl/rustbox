use crate::sys;
use crate::{eprintln, usage};
use rustix::io::Errno;

pub fn run(args: &[&str]) -> i32 {
    let mut recursive = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-r" | "-R" | "--recursive" => recursive = true,
            s if s.starts_with('-') => {
                usage("cp", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if paths.len() < 2 {
        usage("cp", "missing file operand");
        return 1;
    }

    let dest = paths[paths.len() - 1];
    let sources = &paths[..paths.len() - 1];

    if sources.len() > 1 && !sys::is_directory(dest) {
        eprintln(format!("cp: target '{dest}' is not a directory"));
        return 1;
    }

    for src in sources {
        if let Err(e) = copy_one(src, dest, recursive) {
            eprintln(format!("cp: cannot copy '{src}' to '{dest}': {e}"));
            return 1;
        }
    }
    0
}

fn copy_one(src: &str, dest: &str, recursive: bool) -> sys::Result<()> {
    if sys::is_directory(src) {
        if !recursive {
            return Err(Errno::ISDIR);
        }
        let target = if sys::is_directory(dest) {
            join_under(dest, file_name(src))
        } else {
            dest.to_string()
        };
        return sys::copy_dir_recursive(src, &target);
    }

    let target = if sys::is_directory(dest) {
        join_under(dest, file_name(src))
    } else {
        dest.to_string()
    };
    if let Some(parent) = parent_of(&target) {
        sys::mkdir_all(parent)?;
    }
    sys::copy_file(src, &target)
}

fn join_under(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn parent_of(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    path.rfind('/')
        .map(|idx| &path[..idx])
        .filter(|p| !p.is_empty())
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
