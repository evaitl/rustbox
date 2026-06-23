use crate::net::http::{self, Config};
use crate::sys;
use crate::{eprintln, usage};

const DEFAULT_CONFIG: &str = "/etc/thttpd.conf";

pub(crate) struct ArgsConfig {
    pub foreground: bool,
    pub smoke_test: bool,
    pub config_path: String,
    pub port_override: Option<u16>,
    pub dir_override: Option<String>,
}

pub(crate) fn parse_args(args: &[&str]) -> Result<ArgsConfig, i32> {
    let mut foreground = false;
    let mut smoke_test = false;
    let mut config_path = DEFAULT_CONFIG.to_string();
    let mut port_override = None;
    let mut dir_override = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" => foreground = true,
            "-t" => smoke_test = true,
            "-c" => {
                i += 1;
                if i >= args.len() {
                    usage("thttpd", "option requires an argument -- 'c'");
                    return Err(1);
                }
                config_path = args[i].to_string();
            }
            "-p" => {
                i += 1;
                if i >= args.len() {
                    usage("thttpd", "option requires an argument -- 'p'");
                    return Err(1);
                }
                port_override = Some(match args[i].parse() {
                    Ok(port) => port,
                    Err(_) => {
                        usage("thttpd", "invalid port");
                        return Err(1);
                    }
                });
            }
            "-d" => {
                i += 1;
                if i >= args.len() {
                    usage("thttpd", "option requires an argument -- 'd'");
                    return Err(1);
                }
                dir_override = Some(args[i].to_string());
            }
            "-h" | "--help" => {
                usage(
                    "thttpd",
                    "usage: thttpd [-f] [-t] [-c CONF] [-p PORT] [-d DIR]",
                );
                return Err(0);
            }
            s if s.starts_with('-') => {
                usage("thttpd", &format!("invalid option -- '{s}'"));
                return Err(1);
            }
            s => {
                usage("thttpd", &format!("unexpected argument -- '{s}'"));
                return Err(1);
            }
        }
        i += 1;
    }

    Ok(ArgsConfig {
        foreground,
        smoke_test,
        config_path,
        port_override,
        dir_override,
    })
}

pub fn run(args: &[&str]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };

    let mut cfg = if sys::exists(&parsed.config_path) {
        match http::load_config(&parsed.config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln(format!("thttpd: {}: {e}", parsed.config_path));
                return 1;
            }
        }
    } else if parsed.config_path == DEFAULT_CONFIG {
        Config::default()
    } else {
        eprintln(format!(
            "thttpd: {}: No such file or directory",
            parsed.config_path
        ));
        return 1;
    };

    if let Some(port) = parsed.port_override {
        cfg.port = port;
    }
    if let Some(dir) = parsed.dir_override {
        cfg.cgidir = format!("{}/cgi-bin", dir.trim_end_matches('/'));
        cfg.dir = dir;
    }

    if parsed.smoke_test {
        return match http::smoke_test(cfg) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("thttpd: smoke test failed: {e}"));
                1
            }
        };
    }

    if !parsed.foreground {
        if let Err(e) = sys::daemonize() {
            eprintln(format!("thttpd: {e}"));
            return 1;
        }
    }

    eprintln(format!(
        "thttpd: serving {} on port {} (cgi {})",
        cfg.dir, cfg.port, cfg.cgidir
    ));

    match http::serve(cfg) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("thttpd: {e}"));
            1
        }
    }
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_parse_args(input: &str) {
    let args: Vec<String> = input
        .split_whitespace()
        .take(64)
        .map(String::from)
        .collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let _ = parse_args(&refs);
}
