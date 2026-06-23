use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut all = false;
    let mut values_only = false;
    let mut names_only = false;
    let mut ignore_errors = false;
    let mut operands: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-a" | "--all" => all = true,
            "-n" | "--values" => values_only = true,
            "-N" | "--names" => names_only = true,
            "-w" | "--write" => {
                usage("sysctl", "use key=value to write");
                return 1;
            }
            "-e" | "--ignore" => ignore_errors = true,
            "-q" | "--quiet" => {}
            "-b" | "--binary" => {}
            "-p" | "--load" => {
                usage("sysctl", "load from file not supported");
                return 1;
            }
            s if s.starts_with('-') => {
                usage("sysctl", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => operands.push(s),
        }
    }

    if all {
        return print_all(values_only, names_only);
    }

    if operands.is_empty() {
        usage("sysctl", "missing key");
        return 1;
    }

    let mut status = 0;
    for operand in operands {
        if let Some((key, value)) = operand.split_once('=') {
            if let Err(e) = sys::write_sysctl(key, value) {
                if !ignore_errors {
                    eprintln(format!("sysctl: cannot set {key}: {e}"));
                    status = 1;
                }
            }
            continue;
        }
        match sys::read_sysctl(operand) {
            Ok(value) => {
                if names_only {
                    println!("{operand}");
                } else if values_only {
                    println!("{value}");
                } else {
                    println!("{operand} = {value}");
                }
            }
            Err(e) => {
                if !ignore_errors {
                    eprintln(format!("sysctl: cannot read '{operand}': {e}"));
                    status = 1;
                }
            }
        }
    }
    status
}

fn print_all(values_only: bool, names_only: bool) -> i32 {
    let mut entries = Vec::new();
    if let Err(e) = sys::walk_sysctl("", &mut entries) {
        usage("sysctl", &e.to_string());
        return 1;
    }
    for (key, value) in entries {
        if names_only {
            println!("{key}");
        } else if values_only {
            println!("{value}");
        } else {
            println!("{key} = {value}");
        }
    }
    0
}
