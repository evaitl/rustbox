use crate::sys;
use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    let mut clear_env = false;
    let mut vars: Vec<(String, String)> = Vec::new();
    let mut command_start = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-i" => clear_env = true,
            s if s.contains('=') => {
                if let Some((key, value)) = s.split_once('=') {
                    vars.push((key.to_string(), value.to_string()));
                }
            }
            s if s.starts_with('-') => {
                usage("env", &format!("invalid option -- '{s}'"));
                return 1;
            }
            _ => {
                command_start = Some(i);
                break;
            }
        }
        i += 1;
    }

    if let Some(start) = command_start {
        let program = args[start];
        let cmd_args: Vec<&str> = args[start + 1..].to_vec();
        return run_with_env(clear_env, &vars, program, &cmd_args);
    }

    for (key, value) in std::env::vars() {
        if !clear_env {
            println!("{key}={value}");
        }
    }
    for (key, value) in vars {
        println!("{key}={value}");
    }
    0
}

fn run_with_env(clear_env: bool, vars: &[(String, String)], program: &str, args: &[&str]) -> i32 {
    match unsafe { rustix::runtime::kernel_fork() } {
        Ok(rustix::runtime::Fork::Child(_)) => {
            if clear_env {
                for (key, _) in std::env::vars() {
                    std::env::remove_var(key);
                }
            }
            for (key, value) in vars {
                std::env::set_var(key, value);
            }
            let prog = std::ffi::CString::new(program).unwrap();
            let c_args: Vec<std::ffi::CString> = std::iter::once(prog.clone())
                .chain(args.iter().map(|s| std::ffi::CString::new(*s).unwrap()))
                .collect();
            let mut arg_ptrs: Vec<*const u8> = c_args.iter().map(|s| s.as_ptr().cast()).collect();
            arg_ptrs.push(std::ptr::null());
            let env_ptrs = build_env_ptrs();
            let _ = unsafe {
                rustix::runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr())
            };
            rustix::runtime::exit_group(127);
        }
        Ok(rustix::runtime::Fork::ParentOf(pid)) => sys::wait_pid(pid).unwrap_or(1),
        Err(_) => 1,
    }
}

fn build_env_ptrs() -> Vec<*const u8> {
    let env: Vec<std::ffi::CString> = std::env::vars()
        .map(|(k, v)| std::ffi::CString::new(format!("{k}={v}")).unwrap())
        .collect();
    let mut ptrs: Vec<*const u8> = env.iter().map(|s| s.as_ptr().cast()).collect();
    ptrs.push(std::ptr::null());
    ptrs
}
