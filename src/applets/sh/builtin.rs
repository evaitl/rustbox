use super::parse::Word;
use super::Shell;
use super::SHELL_NAME;
use crate::usage;

pub enum BuiltinResult {
    Exit(i32),
    Exec(Vec<String>),
    Status(i32),
    Return(i32),
}

pub fn run_builtin(shell: &mut Shell, argv: &[String]) -> BuiltinResult {
    if argv.is_empty() {
        return BuiltinResult::Status(0);
    }
    match argv[0].as_str() {
        ":" | "true" => BuiltinResult::Status(0),
        "false" => BuiltinResult::Status(1),
        "cd" => BuiltinResult::Status(shell.builtin_cd(argv)),
        "pwd" => BuiltinResult::Status(shell.builtin_pwd()),
        "echo" => BuiltinResult::Status(shell.builtin_echo(argv)),
        "exit" => BuiltinResult::Exit(shell.builtin_exit(argv)),
        "export" => BuiltinResult::Status(shell.builtin_export(argv)),
        "unset" => BuiltinResult::Status(shell.builtin_unset(argv)),
        "set" => BuiltinResult::Status(shell.builtin_set(argv)),
        "shift" => BuiltinResult::Status(shell.builtin_shift(argv)),
        "read" => BuiltinResult::Status(shell.builtin_read(argv)),
        "umask" => BuiltinResult::Status(shell.builtin_umask(argv)),
        "exec" => BuiltinResult::Exec(argv[1..].to_vec()),
        "." | "source" => BuiltinResult::Status(shell.builtin_source(argv)),
        "eval" => BuiltinResult::Status(shell.builtin_eval(argv)),
        "wait" => BuiltinResult::Status(shell.builtin_wait(argv)),
        "break" => BuiltinResult::Status(shell.builtin_break(argv)),
        "continue" => BuiltinResult::Status(shell.builtin_continue(argv)),
        "test" | "[" => BuiltinResult::Status(shell.builtin_test(argv)),
        "trap" => BuiltinResult::Status(shell.builtin_trap(argv)),
        "local" => BuiltinResult::Status(shell.builtin_local(argv)),
        "return" => BuiltinResult::Return(shell.builtin_return(argv)),
        _ => BuiltinResult::Status(127),
    }
}

pub fn is_pipeline_builtin(name: &str) -> bool {
    matches!(
        name,
        ":" | "true" | "false" | "echo" | "read" | "test" | "["
    )
}

pub fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        ":" | "true"
            | "false"
            | "cd"
            | "pwd"
            | "echo"
            | "exit"
            | "export"
            | "unset"
            | "set"
            | "shift"
            | "read"
            | "umask"
            | "exec"
            | "."
            | "source"
            | "eval"
            | "wait"
            | "break"
            | "continue"
            | "test"
            | "["
            | "trap"
            | "local"
            | "return"
    )
}

impl Shell {
    pub fn builtin_cd(&mut self, argv: &[String]) -> i32 {
        let home = self.var_or("HOME", "/");
        let target = argv.get(1).map(String::as_str).unwrap_or(home.as_str());
        let path = if target == "~" {
            home
        } else if let Some(rest) = target.strip_prefix("~/") {
            join_path(&home, rest)
        } else {
            target.to_string()
        };
        match rustix::process::chdir(&path) {
            Ok(()) => 0,
            Err(e) => {
                self.eprintln(format!("cd: {path}: {e}"));
                1
            }
        }
    }

    pub fn builtin_pwd(&self) -> i32 {
        match crate::sys::current_dir() {
            Ok(dir) => {
                println!("{dir}");
                0
            }
            Err(e) => {
                self.eprintln(format!("pwd: {e}"));
                1
            }
        }
    }

    pub fn builtin_echo(&self, argv: &[String]) -> i32 {
        let mut no_newline = false;
        let mut start = 1usize;
        for (i, arg) in argv.iter().enumerate().skip(1) {
            if arg == "-n" {
                no_newline = true;
                start = i + 1;
            } else {
                break;
            }
        }
        let output = argv[start..].join(" ");
        if no_newline {
            print!("{output}");
        } else {
            println!("{output}");
        }
        0
    }

    pub fn builtin_exit(&self, argv: &[String]) -> i32 {
        if argv.len() > 2 {
            crate::usage("exit", "too many arguments");
            return 2;
        }
        if argv.len() == 2 {
            match argv[1].parse::<i32>() {
                Ok(n) => n,
                Err(_) => {
                    crate::usage("exit", "numeric argument required");
                    2
                }
            }
        } else {
            self.last_status
        }
    }

    pub fn builtin_export(&mut self, argv: &[String]) -> i32 {
        if argv.len() == 1 {
            for (key, value) in self.vars.iter() {
                if self.exported.contains(key) {
                    println!("export {key}=\"{value}\"");
                }
            }
            return 0;
        }
        for arg in &argv[1..] {
            if let Some((name, value)) = split_assign(arg) {
                self.set_var(name, value);
                self.exported.insert(name.to_string());
            } else {
                self.exported.insert(arg.clone());
            }
        }
        0
    }

    pub fn builtin_unset(&mut self, argv: &[String]) -> i32 {
        for name in &argv[1..] {
            self.vars.remove(name);
            self.exported.remove(name);
        }
        0
    }

    pub fn builtin_set(&mut self, argv: &[String]) -> i32 {
        let mut i = 1usize;
        while i < argv.len() {
            match argv[i].as_str() {
                "-e" => self.errexit = true,
                "+e" => self.errexit = false,
                "-x" => self.xtrace = true,
                "+x" => self.xtrace = false,
                "-u" => self.nounset = true,
                "+u" => self.nounset = false,
                "-o" => {
                    i += 1;
                    if i >= argv.len() {
                        return 2;
                    }
                    match argv[i].as_str() {
                        "errexit" => self.errexit = true,
                        "xtrace" => self.xtrace = true,
                        "nounset" => self.nounset = true,
                        "pipefail" => self.pipefail = true,
                        _ => return 2,
                    }
                }
                "+o" => {
                    i += 1;
                    if i >= argv.len() {
                        return 2;
                    }
                    match argv[i].as_str() {
                        "errexit" => self.errexit = false,
                        "xtrace" => self.xtrace = false,
                        "nounset" => self.nounset = false,
                        "pipefail" => self.pipefail = false,
                        _ => return 2,
                    }
                }
                "--" => {
                    i += 1;
                    self.positional = vec![SHELL_NAME.to_string()];
                    self.positional.extend(argv[i..].iter().cloned());
                    return 0;
                }
                arg if arg.starts_with('-') => return 2,
                _ => {
                    self.positional = vec![SHELL_NAME.to_string()];
                    self.positional.extend(argv[i..].iter().cloned());
                    return 0;
                }
            }
            i += 1;
        }
        if argv.len() == 1 {
            for (key, value) in &self.vars {
                if self.exported.contains(key) {
                    println!("export {key}=\"{value}\"");
                } else {
                    println!("{key}={value}");
                }
            }
        }
        0
    }

    pub fn builtin_shift(&mut self, argv: &[String]) -> i32 {
        let n = if argv.len() > 1 {
            match argv[1].parse::<usize>() {
                Ok(n) => n,
                Err(_) => return 1,
            }
        } else {
            1
        };
        if self.positional.len() <= n {
            self.positional = vec![self.positional[0].clone()];
            return 0;
        }
        let keep = self.positional[0].clone();
        self.positional = std::iter::once(keep)
            .chain(self.positional[n + 1..].iter().cloned())
            .collect();
        0
    }

    pub fn builtin_read(&mut self, argv: &[String]) -> i32 {
        if argv.len() > 2 {
            return 2;
        }
        let name = argv.get(1).map(String::as_str).unwrap_or("REPLY");
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => return 1,
            Err(_) => return 1,
            Ok(_) => {}
        }
        let line = line.trim_end_matches(['\n', '\r']).to_string();
        self.set_var(name, line);
        0
    }

    pub fn builtin_umask(&self, argv: &[String]) -> i32 {
        if argv.len() == 1 {
            let old = rustix::process::umask(ModeBits::from_raw_mode(0));
            let _ = rustix::process::umask(old);
            println!("{:03o}", old.bits() & 0o777);
            return 0;
        }
        match parse_umask(&argv[1]) {
            Some(bits) => {
                rustix::process::umask(bits);
                0
            }
            None => 1,
        }
    }

    pub fn builtin_source(&mut self, argv: &[String]) -> i32 {
        let Some(path) = argv.get(1) else {
            return 2;
        };
        self.run_file(path)
    }

    pub fn builtin_eval(&mut self, argv: &[String]) -> i32 {
        let script = argv[1..].join(" ");
        self.run_script(&script)
    }

    pub fn builtin_wait(&mut self, argv: &[String]) -> i32 {
        if argv.len() > 1 {
            return 2;
        }
        let mut status = 0;
        while let Some(pid) = self.jobs.pop() {
            match crate::sys::wait_pid(pid) {
                Ok(st) => status = st,
                Err(_) => status = 1,
            }
        }
        status
    }

    pub fn builtin_break(&mut self, argv: &[String]) -> i32 {
        if argv.len() > 2 {
            usage("break", "too many arguments");
            return 1;
        }
        self.loop_break = true;
        0
    }

    pub fn builtin_continue(&mut self, argv: &[String]) -> i32 {
        if argv.len() > 2 {
            usage("continue", "too many arguments");
            return 1;
        }
        self.loop_continue = true;
        0
    }

    pub fn builtin_test(&self, argv: &[String]) -> i32 {
        let raw: Vec<&str> = argv.iter().map(String::as_str).collect();
        let args = if raw.first() == Some(&"[") {
            crate::applets::test_::normalize_bracket(&raw[1..])
        } else if raw.first() == Some(&"test") {
            &raw[1..]
        } else {
            raw.as_slice()
        };
        match crate::applets::test_::eval_args(args) {
            Ok(true) => 0,
            Ok(false) => 1,
            Err(crate::applets::test_::TestError::Syntax) => 2,
        }
    }

    pub fn builtin_trap(&mut self, argv: &[String]) -> i32 {
        if argv.len() == 1 {
            for (sig, cmd) in &self.traps {
                if cmd != "-" {
                    println!("trap -- '{cmd}' {sig}");
                }
            }
            return 0;
        }
        if argv.len() < 3 {
            return 2;
        }
        let (cmd, signals) = if argv[1] == "-" {
            ("-", &argv[2..])
        } else {
            (argv[1].as_str(), &argv[2..])
        };
        if signals.is_empty() {
            return 2;
        }
        for sig in signals {
            let name = normalize_trap_signal(sig);
            if name.is_empty() {
                return 2;
            }
            if cmd == "-" {
                self.traps.remove(name);
                if let Some(num) = trap_signal_number(name) {
                    super::trap::reset_handler(num);
                }
            } else {
                self.traps.insert(name.to_string(), cmd.to_string());
                if let Some(num) = trap_signal_number(name) {
                    super::trap::set_handler(num);
                }
            }
        }
        0
    }

    pub fn builtin_local(&mut self, argv: &[String]) -> i32 {
        if argv.len() < 2 {
            return 2;
        }
        for arg in &argv[1..] {
            if let Some((name, value)) = split_assign(arg) {
                self.set_local(name, value);
            } else {
                self.set_local(arg, self.var_or(arg, ""));
            }
        }
        0
    }

    pub fn builtin_return(&self, argv: &[String]) -> i32 {
        if argv.len() > 2 {
            return 2;
        }
        if argv.len() == 2 {
            argv[1].parse::<i32>().unwrap_or(2)
        } else {
            self.last_status
        }
    }

    pub fn expand_words(&mut self, words: &[Word]) -> super::expand::ExpandResult<Vec<String>> {
        super::expand::expand_argv_words(self, words)
    }
}

use rustix::fs::Mode as ModeBits;

fn split_assign(arg: &str) -> Option<(&str, String)> {
    let (name, value) = arg.split_once('=')?;
    if name.is_empty() {
        return None;
    }
    Some((name, value.to_string()))
}

fn parse_umask(s: &str) -> Option<ModeBits> {
    let value = u32::from_str_radix(s, 8).ok()?;
    Some(ModeBits::from_raw_mode(value))
}

fn join_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn normalize_trap_signal(sig: &str) -> &str {
    match sig {
        "SIGINT" | "INT" => "INT",
        "SIGHUP" | "HUP" => "HUP",
        "SIGTERM" | "TERM" => "TERM",
        "EXIT" => "EXIT",
        other if other.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => other,
        _ => "",
    }
}

fn trap_signal_number(name: &str) -> Option<i32> {
    match name {
        "INT" => Some(libc::SIGINT),
        "HUP" => Some(libc::SIGHUP),
        "TERM" => Some(libc::SIGTERM),
        _ => None,
    }
}
