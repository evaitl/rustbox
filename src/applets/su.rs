use crate::passwd_lookup::{self, LookupError, User};
use crate::{eprintln, usage};
use rustix::process::geteuid;
use std::ffi::CString;
use std::path::Path;

pub(crate) struct Config {
    pub login: bool,
    pub preserve_env: bool,
    pub shell: Option<String>,
    pub command: Option<String>,
    pub user: String,
    pub argv: Vec<String>,
}

pub(crate) fn parse_args(args: &[&str]) -> Result<Config, i32> {
    let mut login = false;
    let mut preserve_env = false;
    let mut shell: Option<String> = None;
    let mut command: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-" => {
                login = true;
            }
            "-l" => login = true,
            "-m" | "-p" => preserve_env = true,
            "-s" => {
                i += 1;
                if i >= args.len() {
                    usage("su", "option requires an argument -- 's'");
                    return Err(1);
                }
                shell = Some(args[i].to_string());
            }
            "-c" => {
                i += 1;
                if i >= args.len() {
                    usage("su", "option requires an argument -- 'c'");
                    return Err(1);
                }
                command = Some(args[i].to_string());
            }
            "-h" | "--help" => {
                usage(
                    "su",
                    "usage: su [-lmp] [-s SHELL] [-c CMD] [-] USER [ARGS...]",
                );
                return Err(0);
            }
            s if s.starts_with('-') => {
                usage("su", &format!("invalid option -- '{s}'"));
                return Err(1);
            }
            s => {
                let user = s.to_string();
                i += 1;
                let argv = args[i..].iter().map(|s| (*s).to_string()).collect();
                return Ok(Config {
                    login,
                    preserve_env,
                    shell,
                    command,
                    user,
                    argv,
                });
            }
        }
        i += 1;
    }

    usage("su", "missing USER operand");
    Err(1)
}

pub fn run(args: &[&str]) -> i32 {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(code) => return code,
    };

    if !geteuid().is_root() {
        eprintln("su: must be suid to work properly");
        return 1;
    }

    let user =
        match passwd_lookup::lookup_user(&config.user, Path::new(passwd_lookup::DEFAULT_PASSWD)) {
            Ok(user) => user,
            Err(LookupError::NoPasswd) => {
                eprintln(format!(
                    "su: {}: no such file",
                    passwd_lookup::DEFAULT_PASSWD
                ));
                return 1;
            }
            Err(LookupError::UnknownUser) => {
                eprintln(format!("su: unknown user {}", config.user));
                return 1;
            }
        };

    if let Err(err) = passwd_lookup::drop_user(&user) {
        eprintln(format!("su: {err}"));
        return 1;
    }

    apply_user_env(&user, &config);

    if let Some(cmd) = &config.command {
        let shell = effective_shell(&user, config.shell.as_deref());
        return exec_command(&shell, cmd);
    }

    if !config.argv.is_empty() {
        let argv: Vec<&str> = config.argv.iter().map(String::as_str).collect();
        return exec_command_argv(&argv);
    }

    let shell = effective_shell(&user, config.shell.as_deref());
    if config.login {
        if let Err(e) = rustix::process::chdir(user.home.as_str()) {
            eprintln(format!("su: cannot change directory to {}: {e}", user.home));
            return 1;
        }
        exec_login_shell(&shell)
    } else {
        exec_command_argv(&[&shell])
    }
}

fn effective_shell(user: &User, override_shell: Option<&str>) -> String {
    override_shell
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            if user.shell.is_empty() || user.shell == "/bin/false" {
                Some("/bin/rash".to_string())
            } else {
                Some(user.shell.clone())
            }
        })
        .unwrap_or_else(|| "/bin/rash".to_string())
}

fn apply_user_env(user: &User, config: &Config) {
    if !config.preserve_env {
        clear_non_essential_env();
    }
    std::env::set_var("HOME", &user.home);
    std::env::set_var("USER", &user.name);
    std::env::set_var("LOGNAME", &user.name);
    let shell = effective_shell(user, config.shell.as_deref());
    std::env::set_var("SHELL", &shell);
    if !config.preserve_env {
        std::env::set_var("PATH", "/bin:/sbin");
    }
}

fn clear_non_essential_env() {
    let keep = ["HOME", "USER", "LOGNAME", "SHELL", "PATH", "TERM"];
    let keys: Vec<String> = std::env::vars()
        .map(|(key, _)| key)
        .filter(|key| !keep.contains(&key.as_str()))
        .collect();
    for key in keys {
        std::env::remove_var(key);
    }
}

fn exec_command(shell: &str, command: &str) -> i32 {
    exec_command_argv(&[shell, "-c", command])
}

fn exec_command_argv(argv: &[&str]) -> i32 {
    match crate::sys::exec_argv(argv) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("su: cannot execute '{}': {e}", argv[0]));
            127
        }
    }
}

fn exec_login_shell(shell: &str) -> i32 {
    let base = shell.rsplit('/').next().unwrap_or(shell);
    let argv0 = format!("-{base}");
    let prog = CString::new(shell).expect("shell path");
    let arg0 = CString::new(argv0).expect("login argv0");
    let arg_ptrs = [arg0.as_ptr().cast(), std::ptr::null()];
    let env_ptrs = build_env_ptrs();

    let err =
        unsafe { rustix::runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr()) };
    eprintln(format!("su: cannot execute '{shell}': {err}"));
    127
}

fn build_env_ptrs() -> Vec<*const u8> {
    let env: Vec<CString> = std::env::vars()
        .map(|(k, v)| CString::new(format!("{k}={v}")).expect("env var"))
        .collect();
    let mut ptrs: Vec<*const u8> = env.iter().map(|s| s.as_ptr().cast()).collect();
    ptrs.push(std::ptr::null());
    ptrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_and_command() {
        let cfg = parse_args(&["-c", "id", "nobody"]).unwrap();
        assert_eq!(cfg.user, "nobody");
        assert_eq!(cfg.command.as_deref(), Some("id"));
    }

    #[test]
    fn parses_login_flag() {
        let cfg = parse_args(&["-", "daemon"]).unwrap();
        assert!(cfg.login);
        assert_eq!(cfg.user, "daemon");
    }

    #[test]
    fn parses_exec_argv() {
        let cfg = parse_args(&["nobody", "/bin/true"]).unwrap();
        assert_eq!(cfg.argv, ["/bin/true"]);
    }
}
