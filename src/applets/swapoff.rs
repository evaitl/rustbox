use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut all = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-a" => all = true,
            s if s.starts_with('-') => {
                usage("swapoff", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    if all {
        return swapoff_all();
    }

    if paths.is_empty() {
        usage("swapoff", "usage: swapoff [-a] DEVICE");
        return 1;
    }

    let mut status = 0;
    for path in paths {
        if let Err(e) = sys::swapoff(path) {
            eprintln(format!("swapoff: {path}: {e}"));
            status = 1;
        }
    }
    status
}

fn swapoff_all() -> i32 {
    let devices = match sys::list_swap_devices() {
        Ok(devices) => devices,
        Err(e) => {
            eprintln(format!("swapoff: {e}"));
            return 1;
        }
    };
    let mut status = 0;
    for path in devices {
        if let Err(e) = sys::swapoff(&path) {
            eprintln(format!("swapoff: {path}: {e}"));
            status = 1;
        }
    }
    status
}
