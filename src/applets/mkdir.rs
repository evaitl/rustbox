use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut parents = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-p" | "--parents" => parents = true,
            s if s.starts_with('-') => {
                usage("mkdir", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if paths.is_empty() {
        usage("mkdir", "missing operand");
        return 1;
    }

    for path in paths {
        let result = if parents {
            sys::mkdir_all(path)
        } else {
            sys::mkdir_one(path)
        };
        if let Err(e) = result {
            eprintln(format!("mkdir: cannot create directory '{path}': {e}"));
            return 1;
        }
    }
    0
}
