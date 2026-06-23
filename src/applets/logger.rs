use crate::net::syslog::{self, DEFAULT_SOCKET};
use crate::{eprintln, usage};

const DEFAULT_PRIORITY: u8 = 13; // user.notice

pub fn run(args: &[&str]) -> i32 {
    let mut tag: Option<String> = None;
    let mut priority = DEFAULT_PRIORITY;
    let mut socket_path = DEFAULT_SOCKET.to_string();
    let mut stderr_too = false;
    let mut message_parts: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-t" => {
                i += 1;
                if i >= args.len() {
                    usage("logger", "option requires an argument -- 't'");
                    return 1;
                }
                tag = Some(args[i].to_string());
            }
            "-p" => {
                i += 1;
                if i >= args.len() {
                    usage("logger", "option requires an argument -- 'p'");
                    return 1;
                }
                priority = match parse_priority(args[i]) {
                    Some(p) => p,
                    None => {
                        usage("logger", "invalid priority");
                        return 1;
                    }
                };
            }
            "-s" => stderr_too = true,
            "-h" | "--help" => {
                usage(
                    "logger",
                    "usage: logger [-s] [-t TAG] [-p PRIO] [-S SOCKET] [MESSAGE]",
                );
                return 0;
            }
            "-S" => {
                i += 1;
                if i >= args.len() {
                    usage("logger", "option requires an argument -- 'S'");
                    return 1;
                }
                socket_path = args[i].to_string();
            }
            s if s.starts_with('-') => {
                usage("logger", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => message_parts.push(s),
        }
        i += 1;
    }

    let message = if message_parts.is_empty() {
        match read_stdin_message() {
            Ok(msg) => msg,
            Err(e) => {
                eprintln(format!("logger: {e}"));
                return 1;
            }
        }
    } else {
        message_parts.join(" ")
    };

    if message.is_empty() {
        usage("logger", "missing message");
        return 1;
    }

    if stderr_too {
        if let Some(ref tag) = tag {
            eprintln(format!("{tag}: {message}"));
        } else {
            eprintln(&message);
        }
    }

    match syslog::send_message(&socket_path, priority, tag.as_deref(), message.trim_end()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("logger: {e}"));
            1
        }
    }
}

fn read_stdin_message() -> Result<String, String> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

fn parse_priority(s: &str) -> Option<u8> {
    if let Ok(n) = s.parse::<u8>() {
        return Some(n);
    }
    let (facility, level) = s.split_once('.')?;
    let facility = parse_facility(facility)?;
    let level = parse_level(level)?;
    Some(facility + level)
}

fn parse_facility(name: &str) -> Option<u8> {
    let n = match name.to_ascii_lowercase().as_str() {
        "kern" => 0,
        "user" => 1,
        "mail" => 2,
        "daemon" => 3,
        "auth" => 4,
        "syslog" => 5,
        "lpr" => 6,
        "news" => 7,
        "uucp" => 8,
        "cron" => 9,
        "authpriv" => 10,
        "ftp" => 11,
        "local0" => 16,
        "local1" => 17,
        "local2" => 18,
        "local3" => 19,
        "local4" => 20,
        "local5" => 21,
        "local6" => 22,
        "local7" => 23,
        _ => return None,
    };
    Some(n << 3)
}

fn parse_level(name: &str) -> Option<u8> {
    match name.to_ascii_lowercase().as_str() {
        "emerg" => Some(0),
        "alert" => Some(1),
        "crit" => Some(2),
        "err" => Some(3),
        "warning" | "warn" => Some(4),
        "notice" => Some(5),
        "info" => Some(6),
        "debug" => Some(7),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_priority() {
        assert_eq!(parse_priority("user.notice"), Some(13));
        assert_eq!(parse_priority("daemon.err"), Some(27));
    }

    #[test]
    fn parses_numeric_priority() {
        assert_eq!(parse_priority("42"), Some(42));
    }
}
