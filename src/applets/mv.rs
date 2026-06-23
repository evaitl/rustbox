use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    if args.len() < 2 {
        usage("mv", "missing file operand");
        return 1;
    }

    let dest = args[args.len() - 1];
    let sources = &args[..args.len() - 1];

    if sources.len() > 1 && !sys::is_directory(dest) {
        eprintln(format!("mv: target '{dest}' is not a directory"));
        return 1;
    }

    for src in sources {
        let target = if sys::is_directory(dest) {
            join_under(dest, file_name(src))
        } else {
            dest.to_string()
        };

        if let Err(e) = sys::rename_path(src, &target) {
            eprintln(format!("mv: cannot move '{src}' to '{target}': {e}"));
            return 1;
        }
    }
    0
}

fn join_under(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
