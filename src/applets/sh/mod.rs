mod arith;
mod builtin;
mod exec;
mod expand;
mod lineedit;
mod parse;
mod trap;

pub(crate) const SHELL_NAME: &str = "rash";

use crate::{eprintln, usage};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, IsTerminal};

pub struct Shell {
    pub vars: HashMap<String, String>,
    pub exported: HashSet<String>,
    pub positional: Vec<String>,
    pub last_status: i32,
    pub errexit: bool,
    pub xtrace: bool,
    pub nounset: bool,
    pub pipefail: bool,
    pub jobs: Vec<rustix::process::Pid>,
    pub loop_break: bool,
    pub loop_continue: bool,
    pub functions: HashMap<String, parse::List>,
    pub traps: HashMap<String, String>,
    pub local_scopes: Vec<HashMap<String, String>>,
    pub return_status: Option<i32>,
    cmdsub_depth: u32,
    call_depth: u32,
}

const MAX_CMDSUB_DEPTH: u32 = 128;
pub(crate) const MAX_FUNCTION_DEPTH: u32 = 128;
#[cfg(feature = "fuzzing")]
pub(crate) const MAX_LOOP_ITER: u32 = 1000;

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    pub fn new() -> Self {
        let mut vars = HashMap::new();
        let mut exported = HashSet::new();
        for (key, value) in std::env::vars() {
            exported.insert(key.clone());
            vars.insert(key, value);
        }
        Self {
            vars,
            exported,
            positional: vec![SHELL_NAME.to_string()],
            last_status: 0,
            errexit: false,
            xtrace: false,
            nounset: false,
            pipefail: false,
            jobs: Vec::new(),
            loop_break: false,
            loop_continue: false,
            functions: HashMap::new(),
            traps: HashMap::new(),
            local_scopes: Vec::new(),
            return_status: None,
            cmdsub_depth: 0,
            call_depth: 0,
        }
    }

    pub fn set_var(&mut self, name: &str, value: String) {
        for scope in self.local_scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return;
            }
        }
        self.vars.insert(name.to_string(), value);
    }

    pub fn set_local(&mut self, name: &str, value: String) {
        if let Some(scope) = self.local_scopes.last_mut() {
            scope.insert(name.to_string(), value);
        } else {
            self.vars.insert(name.to_string(), value);
        }
    }

    pub fn push_local_scope(&mut self) {
        self.local_scopes.push(HashMap::new());
    }

    pub fn pop_local_scope(&mut self) {
        self.local_scopes.pop();
    }

    pub fn lookup_var(&self, name: &str) -> Option<&String> {
        for scope in self.local_scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        self.vars.get(name)
    }

    pub fn var_or(&self, name: &str, default: &str) -> String {
        self.lookup_var(name)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    pub fn effective_vars(&self) -> HashMap<String, String> {
        let mut out = self.vars.clone();
        for scope in &self.local_scopes {
            for (k, v) in scope {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    pub fn eprintln(&self, msg: String) {
        eprintln(msg);
    }

    pub fn expand_ctx(&self) -> expand::ExpandCtx<'_> {
        expand::ExpandCtx {
            vars: self.effective_vars(),
            positional: &self.positional,
            last_status: self.last_status,
            nounset: self.nounset,
        }
    }

    pub fn install_signal_handlers(&mut self) {
        trap::install_handlers();
    }
}

pub fn run(args: &[&str]) -> i32 {
    let mut command: Option<&str> = None;
    let mut script: Option<&str> = None;
    let mut interactive = false;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-c" => {
                i += 1;
                if i >= args.len() {
                    usage(SHELL_NAME, "option requires an argument -- 'c'");
                    return 1;
                }
                command = Some(args[i]);
            }
            "-i" => interactive = true,
            s if s.starts_with('-') => {
                usage(SHELL_NAME, &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                if script.is_some() {
                    usage(SHELL_NAME, "too many arguments");
                    return 1;
                }
                script = Some(s);
            }
        }
        i += 1;
    }

    let mut shell = Shell::new();
    shell.install_signal_handlers();

    if let Some(cmd) = command {
        return shell.run_script(cmd);
    }
    if let Some(path) = script {
        shell.positional[0] = path.to_string();
        return shell.run_file(path);
    }
    if io::stdin().is_terminal() || interactive {
        shell.run_interactive()
    } else {
        shell.run_stdin()
    }
}

impl Shell {
    fn run_interactive(&mut self) -> i32 {
        use lineedit::{read_line_editable, History};

        let prompt = self.var_or("PS1", "$ ");
        let histfile = self.var_or("HISTFILE", "");
        let mut history = if histfile.is_empty() {
            History::default()
        } else {
            History::load_file(&histfile)
        };
        loop {
            let line = match read_line_editable(&prompt, &mut history) {
                Ok(Some(line)) => line,
                Ok(None) => {
                    if !histfile.is_empty() {
                        history.save_file(&histfile);
                    }
                    return self.last_status;
                }
                Err(e) => {
                    eprintln(format!("{SHELL_NAME}: read error: {e}"));
                    return 1;
                }
            };
            history.push(&line);
            if !histfile.is_empty() {
                history.save_file(&histfile);
            }
            self.last_status = self.run_script(&format!("{line}\n"));
            if self.errexit && self.last_status != 0 {
                return self.last_status;
            }
        }
    }

    fn run_stdin(&mut self) -> i32 {
        let stdin = io::stdin();
        let mut input = String::new();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    input.push_str(&line);
                    input.push('\n');
                }
                Err(e) => {
                    eprintln(format!("{SHELL_NAME}: read error: {e}"));
                    return 1;
                }
            }
        }
        self.run_script(&input)
    }
}

#[cfg(feature = "fuzzing")]
pub mod fuzz {
    use super::arith;
    use super::parse::Parser;
    use super::Shell;

    const MAX_SCRIPT_LEN: usize = 16 * 1024;

    fn trim_input(input: &str) -> &str {
        if input.len() <= MAX_SCRIPT_LEN {
            return input;
        }
        &input[..MAX_SCRIPT_LEN]
    }

    pub fn rash_parse(input: &str) {
        let input = trim_input(input);
        let _ = Parser::new(input).parse_list();
    }

    pub fn rash_arith(input: &str) {
        let input = trim_input(input);
        let shell = Shell::new();
        let _ = arith::eval(input, &shell.vars);
    }

    pub fn rash_run(input: &str) {
        let input = trim_input(input);
        let mut shell = Shell::new();
        shell.set_var("PATH", String::new());
        shell.set_var("IFS", " \t\n".to_string());
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = shell.run_script(input);
        }));
    }
}
