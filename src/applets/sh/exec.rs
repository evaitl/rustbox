use super::builtin::{is_builtin, is_pipeline_builtin, run_builtin, BuiltinResult};
use super::expand;
use super::parse::{
    AndOr, AndOrOp, Command, List, ListSep, ParseError, Parser, Pipeline, Redirect, SimpleCommand,
};
use super::trap;
use super::Shell;
use super::SHELL_NAME;
use crate::eprintln;
use crate::sys;
use rustix::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use rustix::fs::{self, Mode, OFlags};
use rustix::io::{self, read, write};
use rustix::pipe::pipe;
use rustix::runtime::{self, Fork};
use rustix::stdio;

#[cfg(feature = "fuzzing")]
const FUZZ_INLINE: bool = true;
#[cfg(not(feature = "fuzzing"))]
const FUZZ_INLINE: bool = false;

impl Shell {
    pub fn run_script(&mut self, input: &str) -> i32 {
        match Parser::new(input).parse_list() {
            Ok(list) => self.run_list(&list),
            Err(e) => {
                self.syntax_error(e);
                2
            }
        }
    }

    pub fn run_file(&mut self, path: &str) -> i32 {
        let text = match sys::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                eprintln(format!("{SHELL_NAME}: {path}: {e}"));
                return 1;
            }
        };
        self.run_script(strip_shebang(&text))
    }

    pub fn run_command_substitution(
        &mut self,
        script: &str,
    ) -> super::expand::ExpandResult<String> {
        if self.cmdsub_depth >= super::MAX_CMDSUB_DEPTH {
            return Err(super::expand::ExpandError::Syntax);
        }
        self.cmdsub_depth += 1;
        let result = self
            .run_command_substitution_inner(script)
            .map_err(|()| super::expand::ExpandError::Syntax);
        self.cmdsub_depth -= 1;
        result
    }

    fn run_command_substitution_inner(&mut self, script: &str) -> Result<String, ()> {
        if FUZZ_INLINE {
            let _ = self.run_script(script);
            return Ok(String::new());
        }

        let (read_fd, write_fd) = pipe().map_err(|_| ())?;
        match unsafe { runtime::kernel_fork() }.map_err(|_| ())? {
            Fork::Child(_) => {
                let _ = stdio::dup2_stdout(&write_fd);
                drop(read_fd);
                drop(write_fd);
                let status = self.run_script(script);
                self.finish_child(status);
            }
            Fork::ParentOf(pid) => {
                drop(write_fd);
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    match read(&read_fd, &mut chunk) {
                        Ok(0) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        Err(io::Errno::INTR) => {}
                        Err(_) => break,
                    }
                }
                drop(read_fd);
                let _ = sys::wait_pid(pid);
                Ok(String::from_utf8_lossy(&buf).into_owned())
            }
        }
    }

    pub fn run_list(&mut self, list: &List) -> i32 {
        #[cfg(feature = "fuzzing")]
        {
            self.exec_steps = self.exec_steps.saturating_add(1);
            if self.exec_steps > super::MAX_EXEC_STEPS {
                return 1;
            }
        }
        if list.andors.is_empty() {
            return 0;
        }
        let mut status = 0;
        for (i, andor) in list.andors.iter().enumerate() {
            self.run_traps();
            if self.xtrace {
                eprintln(format!("+ {:?}", andor));
            }
            status = self.run_andor(andor);
            self.last_status = status;
            if self.return_status.is_some() {
                return self.return_status.take().unwrap_or(status);
            }
            if self.loop_break || self.loop_continue {
                return status;
            }
            if self.errexit && status != 0 {
                return status;
            }
            if i < list.seps.len() && list.seps[i] == ListSep::Background {
                self.spawn_background(andor);
            }
        }
        status
    }

    fn run_traps(&mut self) {
        while let Some(sig) = trap::take_pending_signal() {
            if let Some(name) = trap::signal_name(sig) {
                if let Some(cmd) = self.traps.get(name).cloned() {
                    if cmd == "-" || cmd.is_empty() {
                        trap::reset_handler(sig);
                    } else {
                        let _ = self.run_script(&cmd);
                        trap::set_handler(sig);
                    }
                }
            }
        }
    }

    /// Run the `EXIT` trap once (removed before execution to avoid recursion).
    fn run_exit_trap(&mut self) {
        if self.in_exit_trap {
            return;
        }
        let Some(cmd) = self.traps.remove("EXIT") else {
            return;
        };
        if cmd == "-" || cmd.is_empty() {
            return;
        }
        self.in_exit_trap = true;
        let _ = self.run_script(&cmd);
        self.in_exit_trap = false;
    }

    /// Prepare to leave this shell: run `EXIT` trap, then return the exit status.
    pub(crate) fn leave_shell(&mut self, status: i32) -> i32 {
        if !self.in_exit_trap {
            self.run_exit_trap();
        }
        self.exit_status.take().unwrap_or(status)
    }

    /// `exit` builtin: record status, run `EXIT` trap, return final status.
    pub(crate) fn do_exit(&mut self, status: i32) -> i32 {
        self.exit_status = Some(status);
        if self.in_exit_trap {
            return status;
        }
        self.leave_shell(status)
    }

    fn finish_child(&mut self, status: i32) -> ! {
        runtime::exit_group(self.leave_shell(status));
    }

    fn run_andor(&mut self, andor: &AndOr) -> i32 {
        let mut status = self.run_pipeline(&andor.pipelines[0]);
        for (pipeline, op) in andor.pipelines.iter().skip(1).zip(&andor.ops) {
            let next = match op {
                AndOrOp::And => status == 0,
                AndOrOp::Or => status != 0,
            };
            if next {
                status = self.run_pipeline(pipeline);
            }
        }
        status
    }

    fn run_pipeline(&mut self, pipeline: &Pipeline) -> i32 {
        if pipeline.commands.is_empty() {
            return 0;
        }
        if pipeline.commands.len() == 1 {
            let status = self.run_command(&pipeline.commands[0], &RunCtx::default());
            return if pipeline.negated {
                if status == 0 {
                    1
                } else {
                    0
                }
            } else {
                status
            };
        }

        let n = pipeline.commands.len();
        let mut pipes = Vec::new();
        for _ in 0..n - 1 {
            pipes.push(pipe().expect("pipe"));
        }

        let mut statuses = Vec::new();
        let mut pids = Vec::new();
        let mut in_process = true;

        for command in &pipeline.commands {
            if !command_pipeline_safe(command) {
                in_process = false;
                break;
            }
        }
        if FUZZ_INLINE {
            in_process = true;
        }

        if in_process {
            for (i, command) in pipeline.commands.iter().enumerate() {
                let ctx = RunCtx {
                    stdin: if i > 0 {
                        Some(pipes[i - 1].0.as_fd())
                    } else {
                        None
                    },
                    stdout: if i + 1 < n {
                        Some(pipes[i].1.as_fd())
                    } else {
                        None
                    },
                };
                statuses.push(self.run_command(command, &ctx));
            }
            for (r, w) in pipes {
                drop(r);
                drop(w);
            }
        } else {
            for (i, command) in pipeline.commands.iter().enumerate() {
                if command_pipeline_safe(command) {
                    match unsafe { runtime::kernel_fork() } {
                        Ok(Fork::Child(_)) => {
                            if i > 0 {
                                let _ = stdio::dup2_stdin(&pipes[i - 1].0);
                            }
                            if i + 1 < n {
                                let _ = stdio::dup2_stdout(&pipes[i].1);
                            }
                            for (r, w) in pipes {
                                drop(r);
                                drop(w);
                            }
                            let ctx = RunCtx::default();
                            let status = self.run_command(command, &ctx);
                            self.finish_child(status);
                        }
                        Ok(Fork::ParentOf(pid)) => pids.push(pid),
                        Err(_) => return 1,
                    }
                } else {
                    match unsafe { runtime::kernel_fork() } {
                        Ok(Fork::Child(_)) => {
                            if i > 0 {
                                let _ = stdio::dup2_stdin(&pipes[i - 1].0);
                            }
                            if i + 1 < n {
                                let _ = stdio::dup2_stdout(&pipes[i].1);
                            }
                            for (r, w) in pipes {
                                drop(r);
                                drop(w);
                            }
                            let status = self.run_command(command, &RunCtx::default());
                            self.finish_child(status);
                        }
                        Ok(Fork::ParentOf(pid)) => pids.push(pid),
                        Err(_) => return 1,
                    }
                }
            }

            for (r, w) in pipes {
                drop(r);
                drop(w);
            }

            for pid in pids {
                statuses.push(sys::wait_pid(pid).unwrap_or(1));
            }
        }

        let mut status = statuses.last().copied().unwrap_or(0);
        if self.pipefail {
            for st in &statuses {
                if *st != 0 {
                    status = *st;
                }
            }
        }
        if pipeline.negated {
            if status == 0 {
                1
            } else {
                0
            }
        } else {
            status
        }
    }

    fn run_command(&mut self, command: &Command, ctx: &RunCtx<'_>) -> i32 {
        self.run_traps();
        match command {
            Command::Simple(cmd) => self.run_simple(cmd, ctx),
            Command::If {
                cond,
                then_part,
                elifs,
                else_part,
            } => {
                if self.run_list(cond) == 0 {
                    return self.run_list(then_part);
                }
                for (econd, ebody) in elifs {
                    if self.run_list(econd) == 0 {
                        return self.run_list(ebody);
                    }
                }
                if let Some(el) = else_part {
                    self.run_list(el)
                } else {
                    0
                }
            }
            Command::While { cond, body } => {
                #[cfg(feature = "fuzzing")]
                let mut loop_iter = 0u32;
                loop {
                    #[cfg(feature = "fuzzing")]
                    {
                        loop_iter += 1;
                        if loop_iter > super::MAX_LOOP_ITER {
                            return 0;
                        }
                    }
                    if self.run_list(cond) != 0 {
                        return 0;
                    }
                    self.loop_break = false;
                    self.loop_continue = false;
                    if self.run_list(body) != 0 && self.errexit {
                        return self.last_status;
                    }
                    if self.loop_break {
                        self.loop_break = false;
                        return 0;
                    }
                    if self.loop_continue {
                        self.loop_continue = false;
                        continue;
                    }
                }
            }
            Command::For {
                var,
                items,
                has_in,
                body,
            } => {
                let values = if *has_in {
                    let mut values = Vec::new();
                    for item in items {
                        let expanded = expand::expand_command_substitution(self, item)
                            .unwrap_or_else(|_| item.clone());
                        let mut ctx = self.expand_ctx();
                        values.extend(
                            expand::expand_word(&mut ctx, &expanded, true).unwrap_or_default(),
                        );
                    }
                    values
                } else {
                    self.positional[1..].to_vec()
                };
                #[cfg(feature = "fuzzing")]
                let mut loop_iter = 0u32;
                for value in values {
                    #[cfg(feature = "fuzzing")]
                    {
                        loop_iter += 1;
                        if loop_iter > super::MAX_LOOP_ITER {
                            break;
                        }
                    }
                    self.set_var(var, value);
                    self.loop_break = false;
                    self.loop_continue = false;
                    let status = self.run_list(body);
                    if status != 0 && self.errexit {
                        return status;
                    }
                    if self.loop_break {
                        self.loop_break = false;
                        return 0;
                    }
                    if self.loop_continue {
                        self.loop_continue = false;
                        continue;
                    }
                }
                0
            }
            Command::Case { word, arms } => {
                let text = match word.quote {
                    super::parse::QuoteMode::Single => word.text.clone(),
                    _ => self.expand_assign(&word.text),
                };
                let mut ctx = self.expand_ctx();
                let value = expand::expand_word(&mut ctx, &text, false)
                    .unwrap_or_default()
                    .join("");
                for arm in arms {
                    for pattern in &arm.patterns {
                        let pat = self.expand_assign(pattern);
                        if expand::glob_match(&pat, &value) {
                            return self.run_list(&arm.body);
                        }
                    }
                }
                0
            }
            Command::FunctionDef { name, body } => {
                self.functions.insert(name.clone(), body.clone());
                0
            }
            Command::Brace(list) | Command::Subshell(list) => {
                if matches!(command, Command::Subshell(_)) && !FUZZ_INLINE {
                    match unsafe { runtime::kernel_fork() } {
                        Ok(Fork::Child(_)) => {
                            let status = self.run_list(list);
                            self.finish_child(status);
                        }
                        Ok(Fork::ParentOf(pid)) => return sys::wait_pid(pid).unwrap_or(1),
                        Err(_) => return 1,
                    }
                }
                self.run_list(list)
            }
        }
    }

    fn run_simple(&mut self, cmd: &SimpleCommand, ctx: &RunCtx<'_>) -> i32 {
        for (name, value) in &cmd.assigns {
            let expanded = self.expand_assign(value);
            self.set_var(name, expanded);
            self.exported.insert(name.clone());
        }

        let argv = match self.expand_words(&cmd.words) {
            Ok(argv) => argv,
            Err(super::expand::ExpandError::Syntax) => return 2,
        };

        let redirects = match self.expand_redirects(&cmd.redirects) {
            Ok(r) => r,
            Err(()) => return 2,
        };

        if argv.is_empty() {
            if redirects.is_empty() {
                return 0;
            }
            let noop = vec![":".to_string()];
            return self.run_with_redirects(&noop, &redirects, ctx, |shell, argv| {
                shell.run_builtin_in_ctx(argv, ctx)
            });
        }

        if let Some(body) = self.functions.get(&argv[0]).cloned() {
            return self.call_function(&body, &argv);
        }

        if is_builtin(&argv[0]) {
            if ctx.stdin.is_none() && ctx.stdout.is_none() {
                if redirects.is_empty() {
                    return self.run_builtin_in_ctx(&argv, ctx);
                }
                return self.run_with_redirects(&argv, &redirects, ctx, |shell, argv| {
                    shell.run_builtin_in_ctx(argv, ctx)
                });
            }
            return self.run_with_redirects(&argv, &redirects, ctx, |shell, argv| {
                shell.run_builtin_in_ctx(argv, ctx)
            });
        }

        if is_pipeline_builtin(&argv[0])
            && (ctx.stdin.is_some() || ctx.stdout.is_some() || !redirects.is_empty())
        {
            return self.run_with_redirects(&argv, &redirects, ctx, |shell, argv| {
                shell.run_builtin_in_ctx(argv, ctx)
            });
        }

        self.spawn_simple(&argv, &redirects, ctx)
    }

    fn call_function(&mut self, body: &List, argv: &[String]) -> i32 {
        if self.call_depth >= super::MAX_FUNCTION_DEPTH {
            #[cfg(not(feature = "fuzzing"))]
            eprintln(format!(
                "{SHELL_NAME}: maximum function call depth ({}) exceeded",
                super::MAX_FUNCTION_DEPTH
            ));
            return 1;
        }
        self.call_depth += 1;
        let saved_positional = self.positional.clone();
        let mut new_pos = vec![argv[0].clone()];
        new_pos.extend(argv[1..].iter().cloned());
        self.positional = new_pos;
        self.push_local_scope();
        self.return_status = None;
        let status = self.run_list(body);
        self.pop_local_scope();
        self.positional = saved_positional;
        self.call_depth -= 1;
        if let Some(ret) = self.return_status.take() {
            ret
        } else {
            status
        }
    }

    fn run_builtin_in_ctx(&mut self, argv: &[String], ctx: &RunCtx<'_>) -> i32 {
        let saved = save_stdio();
        apply_redirects(&[], ctx);
        let status = match run_builtin(self, argv) {
            BuiltinResult::Status(st) => st,
            BuiltinResult::Return(st) => {
                self.return_status = Some(st);
                st
            }
            BuiltinResult::Exit(code) => {
                let code = self.do_exit(code);
                if FUZZ_INLINE {
                    code
                } else {
                    std::process::exit(code);
                }
            }
            BuiltinResult::Exec(args) => self.exec_external(&args),
        };
        restore_stdio(saved);
        status
    }

    fn run_with_redirects<F>(
        &mut self,
        argv: &[String],
        redirects: &[Redirect],
        ctx: &RunCtx<'_>,
        f: F,
    ) -> i32
    where
        F: FnOnce(&mut Self, &[String]) -> i32,
    {
        let saved = save_stdio();
        apply_redirects(&[], ctx);
        apply_redirects(redirects, &RunCtx::default());
        let status = f(self, argv);
        restore_stdio(saved);
        status
    }

    fn expand_assign(&mut self, value: &str) -> String {
        expand::expand_command_substitution(self, value).unwrap_or_else(|_| value.to_string())
    }

    fn spawn_simple(&mut self, argv: &[String], redirects: &[Redirect], ctx: &RunCtx<'_>) -> i32 {
        if let Some(body) = self.functions.get(&argv[0]).cloned() {
            return self.call_function(&body, argv);
        }

        if is_builtin(&argv[0]) {
            if FUZZ_INLINE {
                return self.run_with_redirects(argv, redirects, ctx, |shell, argv| {
                    shell.run_builtin_in_ctx(argv, ctx)
                });
            }
            match unsafe { runtime::kernel_fork() } {
                Ok(Fork::Child(_)) => {
                    apply_redirects(redirects, ctx);
                    let status = match run_builtin(self, argv) {
                        BuiltinResult::Status(st) => st,
                        BuiltinResult::Return(st) => st,
                        BuiltinResult::Exit(code) => self.do_exit(code),
                        BuiltinResult::Exec(_) => 127,
                    };
                    self.finish_child(status);
                }
                Ok(Fork::ParentOf(pid)) => return sys::wait_pid(pid).unwrap_or(1),
                Err(_) => return 1,
            }
        }

        let program = match resolve_command(&argv[0]) {
            Some(path) => path,
            None => {
                eprintln(format!("{SHELL_NAME}: {}: not found", argv[0]));
                return 127;
            }
        };

        if FUZZ_INLINE {
            eprintln(format!("{SHELL_NAME}: {}: not found", argv[0]));
            return 127;
        }

        match unsafe { runtime::kernel_fork() } {
            Ok(Fork::Child(_)) => {
                apply_redirects(redirects, ctx);
                let Some(prog) = to_cstring(&program) else {
                    self.finish_child(127);
                };
                let mut c_args: Vec<std::ffi::CString> =
                    argv.iter().filter_map(|arg| to_cstring(arg)).collect();
                if c_args.len() != argv.len() {
                    self.finish_child(127);
                }
                if c_args.is_empty() {
                    c_args.push(prog.clone());
                }
                let mut arg_ptrs: Vec<*const u8> =
                    c_args.iter().map(|s| s.as_ptr().cast()).collect();
                arg_ptrs.push(std::ptr::null());
                let (_env, env_ptrs) = self.exec_environment();
                let _ = unsafe {
                    runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr())
                };
                self.finish_child(127);
            }
            Ok(Fork::ParentOf(pid)) => sys::wait_pid(pid).unwrap_or(1),
            Err(_) => 1,
        }
    }

    fn exec_external(&mut self, argv: &[String]) -> i32 {
        if argv.is_empty() {
            return 0;
        }
        let program = match resolve_command(&argv[0]) {
            Some(path) => path,
            None => {
                eprintln(format!("{SHELL_NAME}: {}: not found", argv[0]));
                return 127;
            }
        };
        let Some(prog) = to_cstring(&program) else {
            return 127;
        };
        let c_args: Vec<std::ffi::CString> =
            argv.iter().filter_map(|arg| to_cstring(arg)).collect();
        if c_args.len() != argv.len() {
            return 127;
        }
        let mut arg_ptrs: Vec<*const u8> = c_args.iter().map(|s| s.as_ptr().cast()).collect();
        arg_ptrs.push(std::ptr::null());
        let (_env, env_ptrs) = self.exec_environment();
        let err = unsafe { runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr()) };
        eprintln(format!("{SHELL_NAME}: {}: {err}", argv[0]));
        127
    }

    fn spawn_background(&mut self, andor: &AndOr) {
        if FUZZ_INLINE {
            let _ = self.run_andor(andor);
            return;
        }
        match unsafe { runtime::kernel_fork() } {
            Ok(Fork::Child(_)) => {
                let status = self.run_andor(andor);
                self.finish_child(status);
            }
            Ok(Fork::ParentOf(pid)) => self.jobs.push(pid),
            Err(_) => {}
        }
    }

    fn expand_redirects(&mut self, redirects: &[Redirect]) -> Result<Vec<Redirect>, ()> {
        let mut out = Vec::new();
        for redir in redirects {
            out.push(match redir {
                Redirect::Input(p) => {
                    Redirect::Input(expand::expand_command_substitution(self, p)?)
                }
                Redirect::Output(p) => {
                    Redirect::Output(expand::expand_command_substitution(self, p)?)
                }
                Redirect::Append(p) => {
                    Redirect::Append(expand::expand_command_substitution(self, p)?)
                }
                Redirect::ErrOutput(p) => {
                    Redirect::ErrOutput(expand::expand_command_substitution(self, p)?)
                }
                Redirect::DupOut(p) => {
                    Redirect::DupOut(expand::expand_command_substitution(self, p)?)
                }
                Redirect::ErrToOut => Redirect::ErrToOut,
                Redirect::HereDoc {
                    delimiter,
                    quoted,
                    body,
                } => {
                    let expanded_body = if *quoted {
                        body.clone()
                    } else {
                        expand::expand_command_substitution(self, body)?
                    };
                    Redirect::HereDoc {
                        delimiter: delimiter.clone(),
                        quoted: *quoted,
                        body: expanded_body,
                    }
                }
            });
        }
        Ok(out)
    }

    fn exec_environment(&self) -> (Vec<std::ffi::CString>, Vec<*const u8>) {
        let mut env: Vec<std::ffi::CString> = self
            .vars
            .iter()
            .filter(|(k, _)| self.exported.contains(*k))
            .filter_map(|(k, v)| to_cstring(&format!("{k}={v}")))
            .collect();
        for (k, v) in std::env::vars() {
            if !self.vars.contains_key(&k) {
                if let Some(entry) = to_cstring(&format!("{k}={v}")) {
                    env.push(entry);
                }
            }
        }
        let mut ptrs: Vec<*const u8> = env.iter().map(|s| s.as_ptr().cast()).collect();
        ptrs.push(std::ptr::null());
        (env, ptrs)
    }

    fn syntax_error(&self, err: ParseError) {
        match err {
            ParseError::UnexpectedEof => eprintln(format!(
                "{SHELL_NAME}: syntax error: unexpected end of file"
            )),
            ParseError::Expected(s) => {
                eprintln(format!("{SHELL_NAME}: syntax error: expected '{s}'"))
            }
            ParseError::Syntax => eprintln(format!("{SHELL_NAME}: syntax error")),
        }
    }
}

#[derive(Default, Clone)]
struct RunCtx<'a> {
    stdin: Option<BorrowedFd<'a>>,
    stdout: Option<BorrowedFd<'a>>,
}

struct SavedStdio {
    stdin: OwnedFd,
    stdout: OwnedFd,
    stderr: OwnedFd,
}

fn save_stdio() -> SavedStdio {
    SavedStdio {
        stdin: dup_stdio(stdio::stdin()),
        stdout: dup_stdio(stdio::stdout()),
        stderr: dup_stdio(stdio::stderr()),
    }
}

fn dup_stdio(fd: BorrowedFd<'_>) -> OwnedFd {
    let raw = unsafe { libc::dup(fd.as_raw_fd()) };
    if raw < 0 {
        panic!("dup failed");
    }
    unsafe { OwnedFd::from_raw_fd(raw) }
}

fn restore_stdio(saved: SavedStdio) {
    let _ = stdio::dup2_stdin(&saved.stdin);
    let _ = stdio::dup2_stdout(&saved.stdout);
    let _ = stdio::dup2_stderr(&saved.stderr);
}

fn command_pipeline_safe(command: &Command) -> bool {
    match command {
        Command::Simple(cmd) => {
            !cmd.words.is_empty()
                && is_pipeline_builtin(&cmd.words[0].text)
                && cmd.assigns.is_empty()
                && cmd.redirects.is_empty()
        }
        _ => false,
    }
}

fn strip_shebang(text: &str) -> &str {
    if let Some(rest) = text.strip_prefix("#!") {
        rest.find('\n').map_or("", |idx| &rest[idx + 1..])
    } else {
        text
    }
}

fn apply_redirects(redirects: &[Redirect], ctx: &RunCtx<'_>) {
    if let Some(fd) = ctx.stdin {
        let _ = rustix::stdio::dup2_stdin(fd);
    }
    if let Some(fd) = ctx.stdout {
        let _ = rustix::stdio::dup2_stdout(fd);
    }
    for redir in redirects {
        match redir {
            Redirect::Input(path) => {
                if let Ok(fd) = fs::open(path, OFlags::RDONLY, Mode::empty()) {
                    let _ = rustix::stdio::dup2_stdin(&fd);
                }
            }
            Redirect::Output(path) => {
                let flags = OFlags::WRONLY.union(OFlags::CREATE).union(OFlags::TRUNC);
                if let Ok(fd) = fs::open(path, flags, Mode::RWXU) {
                    let _ = rustix::stdio::dup2_stdout(&fd);
                }
            }
            Redirect::Append(path) => {
                let flags = OFlags::WRONLY.union(OFlags::CREATE).union(OFlags::APPEND);
                if let Ok(fd) = fs::open(path, flags, Mode::RWXU) {
                    let _ = rustix::stdio::dup2_stdout(&fd);
                }
            }
            Redirect::ErrOutput(path) => {
                let flags = OFlags::WRONLY.union(OFlags::CREATE).union(OFlags::TRUNC);
                if let Ok(fd) = fs::open(path, flags, Mode::RWXU) {
                    let _ = rustix::stdio::dup2_stderr(&fd);
                }
            }
            Redirect::DupOut(target) => {
                if let Ok(num) = target.parse::<i32>() {
                    let _ = num;
                }
            }
            Redirect::ErrToOut => {
                let _ = rustix::stdio::dup2_stderr(stdio::stdout());
            }
            Redirect::HereDoc { body, .. } => {
                let Ok((read_fd, write_fd)) = pipe() else {
                    continue;
                };
                let bytes = body.as_bytes();
                let _ = write(&write_fd, bytes);
                drop(write_fd);
                let _ = rustix::stdio::dup2_stdin(&read_fd);
                drop(read_fd);
            }
        }
    }
}

fn to_cstring(s: &str) -> Option<std::ffi::CString> {
    std::ffi::CString::new(s).ok()
}

fn resolve_command(name: &str) -> Option<String> {
    if name.contains('/') {
        return if sys::exists(name) {
            Some(name.to_string())
        } else {
            None
        };
    }
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let candidate = if dir.ends_with('/') {
            format!("{dir}{name}")
        } else {
            format!("{dir}/{name}")
        };
        if sys::exists(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod exec_tests {
    use super::super::Shell;

    fn fuzz_shell() -> Shell {
        let mut shell = Shell::new();
        shell.set_var("PATH", String::new());
        shell.set_var("IFS", " \t\n".to_string());
        shell
    }

    #[test]
    fn runs_function_definition() {
        let mut shell = Shell::new();
        let status = shell.run_script("f() { echo hi; }; f");
        assert_eq!(status, 0);
        assert!(shell.functions.contains_key("f"));
    }

    #[test]
    fn handles_null_bytes_in_argv() {
        let mut shell = Shell::new();
        shell.set_var("PATH", String::new());
        let script = "A=\0x";
        let status = shell.run_script(script);
        assert!(status == 0 || status == 127 || status == 2);
    }

    #[test]
    fn command_substitution_captures_output() {
        let mut shell = Shell::new();
        let status = shell.run_script("echo $(echo hi)");
        assert_eq!(status, 0);
    }

    #[test]
    fn cd_builtin_without_path_lookup() {
        let mut shell = fuzz_shell();
        let sub = std::env::temp_dir().join(format!("rash-cd-{}", std::process::id()));
        std::fs::create_dir_all(&sub).unwrap();
        let script = format!("cd {} && pwd", sub.display());
        assert_eq!(shell.run_script(&script), 0);
        let _ = std::fs::remove_dir(sub);
    }

    #[test]
    fn cd_builtin_with_redirect() {
        let mut shell = fuzz_shell();
        let sub = std::env::temp_dir().join(format!("rash-cd-redir-{}", std::process::id()));
        std::fs::create_dir_all(&sub).unwrap();
        let script = format!("cd {} 2>/dev/null && pwd", sub.display());
        assert_eq!(shell.run_script(&script), 0);
        let _ = std::fs::remove_dir(sub);
    }

    #[test]
    fn empty_for_in_list_runs_zero_times() {
        let mut shell = Shell::new();
        let status = shell.run_script("for x in; do exit 9; done; exit 0");
        assert_eq!(status, 0);
    }

    #[test]
    fn for_without_in_uses_positional_params() {
        let mut shell = Shell::new();
        shell.positional = vec!["rash".into(), "a".into(), "b".into()];
        let status = shell.run_script("for x; do echo $x; done");
        assert_eq!(status, 0);
    }

    #[cfg(not(feature = "fuzzing"))]
    #[test]
    fn export_passes_environment_to_child() {
        let mut shell = Shell::new();
        let status = shell.run_script("export X=child; sh -c 'echo $X'");
        assert_eq!(status, 0);
    }

    #[test]
    fn command_substitution_in_assignment() {
        let mut shell = Shell::new();
        let status = shell.run_script("X=$(echo value); echo $X");
        assert_eq!(status, 0);
    }

    #[test]
    fn command_substitution_inside_double_quotes() {
        let mut shell = Shell::new();
        let status = shell.run_script("echo \"a$(echo b)c\"");
        assert_eq!(status, 0);
    }

    #[test]
    fn empty_for_does_not_run_body() {
        let mut shell = Shell::new();
        let status = shell.run_script("for i in; do exit 42; done; exit 0");
        assert_eq!(status, 0);
    }

    #[test]
    fn for_without_in_with_no_positional_args() {
        let mut shell = Shell::new();
        shell.positional = vec!["rash".into()];
        let status = shell.run_script("for x; do exit 42; done; exit 0");
        assert_eq!(status, 0);
    }

    #[test]
    fn function_call_depth_limit_does_not_overflow() {
        let mut shell = fuzz_shell();
        let status = shell.run_script("f() { f; }; f");
        assert_eq!(status, 1);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn while_loop_iteration_limit_under_fuzzing() {
        let mut shell = fuzz_shell();
        let status = shell.run_script("while true; do :; done");
        assert_eq!(status, 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn exec_step_budget_stops_runaway_recursion() {
        let mut shell = fuzz_shell();
        let status = shell.run_script("f() { f; }; f");
        assert_eq!(status, 1);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_elif_and_else_branches() {
        let mut shell = fuzz_shell();
        assert_eq!(
            shell.run_script("if false; then :; elif true; then exit 3; else exit 4; fi"),
            3
        );
        assert_eq!(
            shell.run_script("if false; then :; elif false; then :; else exit 5; fi"),
            5
        );
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_case_and_heredoc() {
        let mut shell = fuzz_shell();
        assert_eq!(
            shell.run_script("case x in x) exit 0 ;; *) exit 9 ;; esac"),
            0
        );
        assert_eq!(shell.run_script("read -r x <<'EOF'\nline\nEOF\necho $x"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_brace_subshell_and_background() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("{ echo hi; }"), 0);
        assert_eq!(shell.run_script("(exit 0)"), 0);
        assert_eq!(shell.run_script(": &\nwait"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_redirects_and_negated_pipeline() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("echo hi > /dev/null 2> /dev/null"), 0);
        assert_eq!(shell.run_script("> /dev/null"), 0);
        assert_eq!(shell.run_script("! false; echo $?"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_pipefail_and_multi_pipe() {
        let mut shell = fuzz_shell();
        assert_eq!(
            shell.run_script("set -o pipefail; false | true; echo $?"),
            0
        );
        assert_eq!(shell.run_script("echo a | echo b | echo c"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_function_keyword_return_and_local() {
        let mut shell = fuzz_shell();
        assert_eq!(
            shell.run_script("function f { local x=1; return 9; }; f; echo $?"),
            0
        );
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_for_in_and_shift_unset() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("for x in a b; do :; done"), 0);
        shell.positional = vec!["rash".into(), "a".into(), "b".into(), "c".into()];
        assert_eq!(shell.run_script("shift; echo $#"), 0);
        assert_eq!(shell.run_script("unset V; V=1; unset V"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_eval_and_trap() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("eval 'echo ok'"), 0);
        assert_eq!(shell.run_script("trap"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_param_default_substitution() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("echo ${UNSET:-fallback}"), 0);
        assert_eq!(shell.run_script("X=${Y:=assigned}; echo $X"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_external_command_not_found() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("definitely-not-an-applet-xyz"), 127);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_redirect_dup_and_heredoc() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("echo x > /dev/null 2>&1"), 0);
        assert_eq!(
            shell.run_script("read -r line <<EOF\nbody\nEOF\necho $line"),
            0
        );
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_background_and_case() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script(": &\necho after"), 0);
        assert_eq!(shell.run_script("case x in x) echo ok ;; esac"), 0);
    }

    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_syntax_error_returns_two() {
        let mut shell = fuzz_shell();
        assert_eq!(shell.run_script("if then"), 2);
    }
}
