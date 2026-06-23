use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut symbolic = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-s" | "--symbolic" => symbolic = true,
            s if s.starts_with('-') => {
                usage("ln", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if paths.len() < 2 {
        usage("ln", "missing file operand");
        return 1;
    }

    let target = paths[0];
    let links = &paths[1..];

    for link in links {
        let result = if symbolic {
            sys::sym_link(target, link)
        } else {
            sys::hard_link(target, link)
        };
        if let Err(e) = result {
            eprintln(format!(
                "ln: cannot create link '{link}' -> '{target}': {e}"
            ));
            return 1;
        }
    }
    0
}
