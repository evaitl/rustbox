use crate::sys;
use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    let mut show_all = false;
    let mut kernel = false;
    let mut nodename = false;
    let mut release = false;
    let mut sysname = false;
    let mut machine = false;

    for arg in args {
        match *arg {
            "-a" | "--all" => show_all = true,
            "-s" | "--kernel-name" => kernel = true,
            "-n" | "--nodename" => nodename = true,
            "-r" | "--kernel-release" => release = true,
            "-m" | "--machine" => machine = true,
            s if s.starts_with('-') => {
                usage("uname", &format!("invalid option -- '{s}'"));
                return 1;
            }
            _ => {
                usage("uname", "extra operand");
                return 1;
            }
        }
    }

    if show_all {
        kernel = true;
        nodename = true;
        release = true;
        sysname = true;
        machine = true;
    }

    if !kernel && !nodename && !release && !sysname && !machine {
        sysname = true;
    }

    let mut parts = Vec::new();
    if sysname {
        parts.push(read_sysctl("kernel/ostype").unwrap_or_else(|| std::env::consts::OS.into()));
    }
    if nodename {
        parts.push(read_sysctl("kernel/hostname").unwrap_or_else(|| "localhost".into()));
    }
    if release {
        parts.push(read_sysctl("kernel/osrelease").unwrap_or_else(|| "unknown".into()));
    }
    if kernel {
        parts.push(read_sysctl("kernel/ostype").unwrap_or_else(|| std::env::consts::OS.into()));
    }
    if machine {
        parts.push(read_sysctl("kernel/arch").unwrap_or_else(|| std::env::consts::ARCH.into()));
    }

    println!("{}", parts.join(" "));
    0
}

fn read_sysctl(name: &str) -> Option<String> {
    let path = format!("/proc/sys/{name}");
    sys::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
