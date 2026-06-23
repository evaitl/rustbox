use crate::passwd_auth::{self, DEFAULT_AUTH_PASSWD_PATH};
use crate::passwd_lookup::{self, LookupError};
use crate::{eprintln, usage};
use rustix::process::geteuid;
use std::io::{self, Write};
use std::path::Path;

pub(crate) struct Config {
    pub passwd_path: String,
    pub user: Option<String>,
}

pub(crate) fn parse_args(args: &[&str]) -> Result<Config, i32> {
    let mut passwd_path = DEFAULT_AUTH_PASSWD_PATH.to_string();
    let mut user = None;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-f" => {
                i += 1;
                if i >= args.len() {
                    usage("passwd", "option requires an argument -- 'f'");
                    return Err(1);
                }
                passwd_path = args[i].to_string();
            }
            "-h" | "--help" => {
                usage("passwd", "usage: passwd [-f FILE] [USER]");
                return Err(0);
            }
            s if s.starts_with('-') => {
                usage("passwd", &format!("invalid option -- '{s}'"));
                return Err(1);
            }
            s => {
                if user.is_some() {
                    usage("passwd", "too many arguments");
                    return Err(1);
                }
                user = Some(s.to_string());
            }
        }
        i += 1;
    }

    Ok(Config { passwd_path, user })
}

pub fn run(args: &[&str]) -> i32 {
    #[cfg(not(feature = "applet-passwd"))]
    {
        eprintln("passwd: not built (enable applet-passwd feature)");
        return 1;
    }

    #[cfg(feature = "applet-passwd")]
    {
        let config = match parse_args(args) {
            Ok(config) => config,
            Err(code) => return code,
        };

        let target = match resolve_target_user(config.user.as_deref()) {
            Ok(user) => user,
            Err(code) => return code,
        };

        if lookup_user_in_file(&config.passwd_path, &target).is_err() {
            eprintln(format!(
                "passwd: user '{target}' does not exist in {}",
                config.passwd_path
            ));
            return 1;
        }

        let table = passwd_auth::load_auth_passwd(&config.passwd_path);

        let is_root = geteuid().is_root();
        if !is_root {
            let current = match current_username() {
                Some(name) => name,
                None => {
                    eprintln("passwd: cannot determine current user");
                    return 1;
                }
            };
            if current != target {
                eprintln("passwd: permission denied");
                return 1;
            }
            let current_pw = match read_password("Current password: ") {
                Ok(pw) => pw,
                Err(()) => return 1,
            };
            if !table.check(&target, &current_pw) {
                eprintln("passwd: incorrect password");
                return 1;
            }
        }

        let new_pw = match read_password("New password: ") {
            Ok(pw) => pw,
            Err(()) => return 1,
        };
        if new_pw.is_empty() {
            eprintln("passwd: empty password not allowed");
            return 1;
        }
        let confirm = match read_password("Retype new password: ") {
            Ok(pw) => pw,
            Err(()) => return 1,
        };
        if new_pw != confirm {
            eprintln("passwd: passwords do not match");
            return 1;
        }

        let hash = match passwd_auth::hash_password(&new_pw) {
            Ok(hash) => hash,
            Err(_) => {
                eprintln("passwd: failed to hash password");
                return 1;
            }
        };

        if let Err(e) = passwd_auth::update_auth_passwd(&config.passwd_path, &target, &hash) {
            eprintln(format!("passwd: cannot update {}: {e}", config.passwd_path));
            return 1;
        }

        println!("passwd: password updated for {target}");
        0
    }
}

fn lookup_user_in_file(path: &str, user: &str) -> Result<(), LookupError> {
    passwd_lookup::lookup_user(user, Path::new(path)).map(|_| ())
}

fn resolve_target_user(user: Option<&str>) -> Result<String, i32> {
    if let Some(name) = user {
        return Ok(name.to_string());
    }
    current_username().ok_or_else(|| {
        eprintln("passwd: missing USER operand");
        1
    })
}

fn current_username() -> Option<String> {
    if let Ok(name) = std::env::var("LOGNAME") {
        if !name.is_empty() {
            return Some(name);
        }
    }
    if let Ok(name) = std::env::var("USER") {
        if !name.is_empty() {
            return Some(name);
        }
    }
    let uid = geteuid().as_raw();
    passwd_lookup::lookup_user(
        &uid.to_string(),
        std::path::Path::new(passwd_lookup::DEFAULT_PASSWD),
    )
    .ok()
    .map(|user| user.name)
}

fn read_password(prompt: &str) -> Result<String, ()> {
    let mut stdout = io::stdout();
    write!(stdout, "{prompt}").map_err(|_| ())?;
    stdout.flush().map_err(|_| ())?;

    #[cfg(unix)]
    let echo_disabled = disable_echo().ok();

    let mut line = String::new();
    let read_result = io::stdin().read_line(&mut line).map_err(|_| ());

    #[cfg(unix)]
    if let Some(saved) = echo_disabled {
        let _ = restore_echo(&saved);
    }

    writeln!(stdout).map_err(|_| ())?;
    stdout.flush().map_err(|_| ())?;
    read_result?;
    Ok(line.trim_end_matches(['\n', '\r']).to_string())
}

#[cfg(unix)]
fn disable_echo() -> Result<rustix::termios::Termios, ()> {
    use rustix::fd::AsFd;
    use rustix::stdio;
    use rustix::termios::{tcgetattr, tcsetattr, LocalModes, OptionalActions};

    let stdin = stdio::stdin();
    let mut term = tcgetattr(stdin.as_fd()).map_err(|_| ())?;
    let saved = term.clone();
    term.local_modes.remove(LocalModes::ECHO);
    tcsetattr(stdin.as_fd(), OptionalActions::Now, &term).map_err(|_| ())?;
    Ok(saved)
}

#[cfg(unix)]
fn restore_echo(saved: &rustix::termios::Termios) -> Result<(), ()> {
    use rustix::fd::AsFd;
    use rustix::stdio;
    use rustix::termios::{tcsetattr, OptionalActions};

    tcsetattr(stdio::stdin().as_fd(), OptionalActions::Now, saved).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_f_flag() {
        let cfg = parse_args(&["-f", "/tmp/pw", "alice"]).unwrap();
        assert_eq!(cfg.passwd_path, "/tmp/pw");
        assert_eq!(cfg.user.as_deref(), Some("alice"));
    }
}
