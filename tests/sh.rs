mod support;

use support::{bin, Rustbox, TestDir};

#[test]
fn rash_applet_name() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo rash-shell"])
        .stdout();
    assert_eq!(out.trim(), "rash-shell");
}

#[test]
fn sh_arithmetic_expansion() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo $((1+2*3))"])
        .stdout();
    assert_eq!(out.trim(), "7");
}

#[test]
fn sh_break_loop() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "n=0; while [ $n -lt 5 ]; do n=$((n+1)); if [ $n -eq 2 ]; then break; fi; echo $n; done"])
        .stdout();
    assert_eq!(out.trim(), "1");
}

#[test]
fn sh_applet_alias() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "echo sh-alias"])
        .stdout();
    assert_eq!(out.trim(), "sh-alias");
}

#[test]
fn sh_c_runs_command() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "echo hello"])
        .stdout();
    assert_eq!(out.trim(), "hello");
}

#[test]
fn sh_c_exit_status() {
    assert_eq!(
        Rustbox::new().applet("sh").args(["-c", "exit 42"]).status(),
        42
    );
}

#[test]
fn sh_builtin_pwd() {
    let dir = TestDir::new();
    let out = Rustbox::new()
        .current_dir(dir.path())
        .applet("sh")
        .args(["-c", "pwd"])
        .stdout();
    assert_eq!(out.trim(), dir.path().to_string_lossy());
}

#[test]
fn sh_builtin_cd() {
    let dir = TestDir::new();
    let sub = dir.join("sub");
    std::fs::create_dir(&sub).unwrap();
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", &format!("cd {} && pwd", sub.display())])
        .stdout();
    assert_eq!(out.trim(), sub.to_string_lossy());
}

#[test]
fn sh_runs_script() {
    let dir = TestDir::new();
    let bin_path = bin();
    let bin = bin_path.to_str().expect("utf-8 path");
    dir.write("script.sh", &format!("{bin} echo one\n{bin} echo two\n"));

    let out = Rustbox::new()
        .applet("sh")
        .arg(dir.join("script.sh"))
        .stdout();
    assert!(out.contains("one"));
    assert!(out.contains("two"));
}

#[test]
fn sh_rustbox_applet_via_path() {
    let bin_path = bin();
    let bin = bin_path.to_str().expect("utf-8 path");
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", &format!("{bin} echo via-shell")])
        .stdout();
    assert_eq!(out.trim(), "via-shell");
}

#[test]
fn sh_variables_and_expansion() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "FOO=bar; echo $FOO"])
        .stdout();
    assert_eq!(out.trim(), "bar");
}

#[test]
fn sh_pipeline() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "echo hello | wc -c"])
        .stdout();
    assert!(out.trim().parse::<u32>().unwrap() >= 6);
}

#[test]
fn sh_if_then() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "if true; then echo yes; fi"])
        .stdout();
    assert_eq!(out.trim(), "yes");
}

#[test]
fn sh_or_short_circuit() {
    let status = Rustbox::new()
        .applet("sh")
        .args(["-c", "false || exit 7"])
        .status();
    assert_eq!(status, 7);
}

#[test]
fn rash_or_brace_group_with_semicolon() {
    let status = Rustbox::new()
        .applet("sh")
        .args(["-c", "false || { exit 9; }"])
        .status();
    assert_eq!(status, 9);
}

#[test]
fn rash_subshell_with_semicolon_before_close() {
    let status = Rustbox::new()
        .applet("sh")
        .args(["-c", "false || ( exit 8; )"])
        .status();
    assert_eq!(status, 8);
}

#[test]
fn rash_brace_group_multiple_commands() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "{ echo one; echo two; }"])
        .stdout();
    assert_eq!(out.trim(), "one\ntwo");
}

#[test]
fn sh_export() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "export X=exported; sh -c 'echo $X'"])
        .stdout();
    assert_eq!(out.trim(), "exported");
}

#[test]
fn sh_redirect_output() {
    let dir = TestDir::new();
    let path = dir.join("out.txt");
    let status = Rustbox::new()
        .applet("sh")
        .args(["-c", &format!("echo redirected > {}", path.display())])
        .status();
    assert_eq!(status, 0);
    assert_eq!(dir.read("out.txt").trim(), "redirected");
}

#[test]
fn sh_while_loop() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "while false; do echo never; done; echo after"])
        .stdout();
    assert_eq!(out.trim(), "after");
}

#[test]
fn sh_for_loop() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "for x in a b c; do echo $x; done"])
        .stdout();
    assert_eq!(out.trim(), "a\nb\nc");
}

#[test]
fn sh_set_e() {
    let status = Rustbox::new()
        .applet("sh")
        .args(["-c", "set -e; false; echo never"])
        .status();
    assert_ne!(status, 0);
}

#[test]
fn sh_pipefail() {
    let status = Rustbox::new()
        .applet("sh")
        .args(["-c", "set -o pipefail; false | true; echo $?"])
        .stdout();
    assert_eq!(status.trim(), "1");
}

#[test]
fn sh_pipeline_builtin_in_process() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "echo hello | echo world"])
        .stdout();
    assert_eq!(out.trim(), "world");
}

#[test]
fn sh_function_local_return() {
    let out = Rustbox::new()
        .applet("sh")
        .args([
            "-c",
            "f() { local x=inner; echo $x; return 5; }; f; echo $?",
        ])
        .stdout();
    let lines: Vec<&str> = out.trim().lines().collect();
    assert_eq!(lines[0], "inner");
    assert_eq!(lines[1], "5");
}

#[test]
fn sh_function_keyword() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "function greet { echo hi; }; greet"])
        .stdout();
    assert_eq!(out.trim(), "hi");
}

#[test]
fn sh_case_glob() {
    let out = Rustbox::new()
        .applet("sh")
        .args([
            "-c",
            "case foo in f*) echo match ;; *) echo nomatch ;; esac",
        ])
        .stdout();
    assert_eq!(out.trim(), "match");
}

#[test]
fn sh_case_alternate() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "case bar in foo|bar) echo ok ;; *) echo no ;; esac"])
        .stdout();
    assert_eq!(out.trim(), "ok");
}

#[test]
fn sh_heredoc() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "x=world; cat <<EOF\nhello $x\nEOF"])
        .stdout();
    assert_eq!(out.trim(), "hello world");
}

#[test]
fn sh_heredoc_quoted_delim() {
    let out = Rustbox::new()
        .applet("sh")
        .args(["-c", "x=world; cat <<'EOF'\nhello $x\nEOF"])
        .stdout();
    assert_eq!(out.trim(), "hello $x");
}

#[test]
fn sh_trap_builtin_lists() {
    let status = Rustbox::new()
        .applet("sh")
        .args(["-c", "trap 'echo trapped' INT; trap"])
        .status();
    assert_eq!(status, 0);
}

#[test]
fn rash_continue_loop() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "n=0; while [ $n -lt 3 ]; do n=$((n+1)); if [ $n -eq 2 ]; then continue; fi; echo $n; done",
        ])
        .stdout();
    assert_eq!(out.trim(), "1\n3");
}

#[test]
fn rash_elif_branch() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "if false; then echo no; elif true; then echo elif; else echo else; fi",
        ])
        .stdout();
    assert_eq!(out.trim(), "elif");
}

#[test]
fn rash_else_branch() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "if false; then echo no; elif false; then echo elif; else echo yes; fi",
        ])
        .stdout();
    assert_eq!(out.trim(), "yes");
}

#[test]
fn rash_nounset_fails_on_unset_var() {
    let status = Rustbox::new()
        .applet("rash")
        .args(["-c", "set -u; echo $UNSET_VAR"])
        .status();
    assert_ne!(status, 0);
}

#[test]
fn rash_unset_removes_variable() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "X=1; unset X; [ -z \"${X}\" ] && echo cleared"])
        .stdout();
    assert_eq!(out.trim(), "cleared");
}

#[test]
fn rash_shift_positional() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "set -- a b c; shift; echo $1 $2"])
        .stdout();
    assert_eq!(out.trim(), "b c");
}

#[test]
fn rash_status_expansion() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "false; echo $?"])
        .stdout();
    assert_eq!(out.trim(), "1");
}

#[test]
fn rash_arith_modulo() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo $((10 % 3))"])
        .stdout();
    assert_eq!(out.trim(), "1");
}

#[test]
fn rash_arith_unset_var_treated_as_zero() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo $((n + 1))"])
        .stdout();
    assert_eq!(out.trim(), "1");
}

#[test]
fn rash_function_return_sets_status() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "f() { return 3; }; f; echo $?"])
        .stdout();
    assert_eq!(out.trim(), "3");
}

#[test]
fn rash_eval_runs_script() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "eval echo eval_ok"])
        .stdout();
    assert_eq!(out.trim(), "eval_ok");
}

#[test]
fn rash_exit_without_arg_uses_last_status() {
    let status = Rustbox::new()
        .applet("rash")
        .args(["-c", "false; exit"])
        .status();
    assert_eq!(status, 1);
}

#[test]
fn rash_append_redirect() {
    let dir = TestDir::new();
    let path = dir.join("log.txt");
    let status = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            &format!(
                "echo first > {}; echo second >> {}; cat {}",
                path.display(),
                path.display(),
                path.display()
            ),
        ])
        .status();
    assert_eq!(status, 0);
    assert_eq!(dir.read("log.txt").trim(), "first\nsecond");
}

#[test]
fn rash_set_e_exits_on_failed_pipeline() {
    let status = Rustbox::new()
        .applet("rash")
        .args(["-c", "set -e; true | false; echo survived"])
        .status();
    assert_ne!(status, 0);
}

#[test]
fn rash_subshell_assignment_does_not_leak() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "X=outer; (X=inner); echo $X"])
        .stdout();
    assert_eq!(out.trim(), "outer");
}

#[test]
fn rash_test_empty_string() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "[ -z \"\" ] && echo empty"])
        .stdout();
    assert_eq!(out.trim(), "empty");
}

#[test]
fn rash_echo_n_suppresses_newline() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo -n no_nl; echo end"])
        .stdout();
    assert_eq!(out.trim(), "no_nlend");
}

#[test]
fn rash_and_or_short_circuit_chain() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "true && false || echo fallback"])
        .stdout();
    assert_eq!(out.trim(), "fallback");
}

#[test]
fn rash_unset_var_expands_empty() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo x$UNSET"])
        .stdout();
    assert_eq!(out.trim(), "x");
}

#[test]
fn rash_double_quoted_expansion() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "X=hi; echo \"quoted $X\""])
        .stdout();
    assert_eq!(out.trim(), "quoted hi");
}

#[test]
fn rash_single_quoted_literals() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo 'no $expansion'"])
        .stdout();
    assert_eq!(out.trim(), "no $expansion");
}

#[test]
fn rash_external_pipeline() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "/bin/echo one | /bin/echo two"])
        .stdout();
    assert_eq!(out.trim(), "two");
}

#[test]
fn rash_read_builtin() {
    let dir = TestDir::new();
    let path = dir.write("in.txt", "inputline\n");
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            &format!("read line < {}; echo got:$line", path.display()),
        ])
        .stdout();
    assert_eq!(out.trim(), "got:inputline");
}

#[test]
fn rash_source_runs_file() {
    let dir = TestDir::new();
    dir.write("lib.sh", "echo sourced\n");
    let path = dir.join("lib.sh");
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", &format!(". {}", path.display())])
        .stdout();
    assert_eq!(out.trim(), "sourced");
}

#[test]
fn rash_local_shadows_outer_in_function() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "x=outer; f() { local x=inner; echo $x; }; f; echo $x"])
        .stdout();
    let lines: Vec<&str> = out.trim().lines().collect();
    assert_eq!(lines[0], "inner");
    assert_eq!(lines[1], "outer");
}

#[test]
fn rash_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo $(echo substituted)"])
        .stdout();
    assert_eq!(out.trim(), "substituted");
}

#[test]
fn rash_empty_for_in_list() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "for x in; do echo never; done; echo done"])
        .stdout();
    assert_eq!(out.trim(), "done");
}

#[test]
fn rash_for_without_in_uses_positional() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "set -- one two; for x; do echo $x; done"])
        .stdout();
    assert_eq!(out.trim(), "one\ntwo");
}

#[test]
fn rash_command_substitution_assignment() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "X=$(echo assigned); echo $X"])
        .stdout();
    assert_eq!(out.trim(), "assigned");
}

#[test]
fn rash_command_substitution_adjacent_text() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo pre$(echo mid)post"])
        .stdout();
    assert_eq!(out.trim(), "premidpost");
}

#[test]
fn rash_command_substitution_in_double_quotes() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo \"quoted $(echo inner)\""])
        .stdout();
    assert_eq!(out.trim(), "quoted inner");
}

#[test]
fn rash_command_substitution_multiline_output() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo $(echo multi; echo lines)"])
        .stdout();
    assert!(out.contains("multi"));
    assert!(out.contains("lines"));
}

#[test]
fn rash_single_quoted_preserves_dollar_paren() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo '$(literal)'"])
        .stdout();
    assert_eq!(out.trim(), "$(literal)");
}

#[test]
fn rash_single_quoted_empty_word() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo ''"])
        .stdout();
    assert_eq!(out.trim(), "");
}

#[test]
fn rash_empty_for_in_list_with_space() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "for x in ; do echo never; done; echo after"])
        .stdout();
    assert_eq!(out.trim(), "after");
}

#[test]
fn rash_for_without_in_empty_positional() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "set --; for x; do echo never; done; echo after"])
        .stdout();
    assert_eq!(out.trim(), "after");
}

#[test]
fn rash_for_in_single_item() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "for x in only; do echo $x; done"])
        .stdout();
    assert_eq!(out.trim(), "only");
}

#[test]
fn rash_export_visible_in_single_quoted_child_script() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "export V=from_parent; sh -c 'echo $V'"])
        .stdout();
    assert_eq!(out.trim(), "from_parent");
}

#[test]
fn rash_external_pipeline_three_stage() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "/bin/echo a | /bin/echo b | /bin/echo c"])
        .stdout();
    assert_eq!(out.trim(), "c");
}

#[test]
fn rash_pipeline_external_to_builtin() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "/bin/echo hi | echo there"])
        .stdout();
    assert_eq!(out.trim(), "there");
}

#[test]
fn rash_mixed_quoting_on_one_line() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "X=expanded; echo 'lit $X' $X"])
        .stdout();
    assert_eq!(out.trim(), "lit $X expanded");
}

// --- POSIX feature combinations (rash) ---

#[test]
fn rash_for_splits_command_substitution_on_spaces() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "for x in $(echo a b c); do echo $x; done"])
        .stdout();
    assert_eq!(out.trim(), "a\nb\nc");
}

#[test]
fn rash_for_splits_command_substitution_on_newlines() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "for x in $(echo one; echo two; echo three); do echo $x; done",
        ])
        .stdout();
    assert_eq!(out.trim(), "one\ntwo\nthree");
}

#[test]
fn rash_quoted_command_substitution_not_split() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "set -- \"$(echo a b c)\"; echo $#; echo \"$1\""])
        .stdout();
    let lines: Vec<&str> = out.trim().lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "a b c");
}

#[test]
fn rash_echo_expands_unquoted_command_substitution_to_words() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "echo $(echo a b c)"])
        .stdout();
    assert_eq!(out.trim(), "a b c");
}

#[test]
fn rash_for_body_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "for x in a b; do echo $(echo item-$x); done"])
        .stdout();
    assert_eq!(out.trim(), "item-a\nitem-b");
}

#[test]
fn rash_for_list_mixed_literals_and_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "for x in a $(echo b) c; do echo $x; done"])
        .stdout();
    assert_eq!(out.trim(), "a\nb\nc");
}

#[test]
fn rash_for_single_word_from_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "for x in $(echo solo); do echo \"[$x]\"; done"])
        .stdout();
    assert_eq!(out.trim(), "[solo]");
}

#[test]
fn rash_for_builds_string_via_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "out=\"\"; for i in 1 2 3; do out=\"$out$(echo $i)\"; done; echo $out",
        ])
        .stdout();
    assert_eq!(out.trim(), "123");
}

#[test]
fn rash_for_continue_with_command_substitution_in_body() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "for n in 1 2 3; do if [ $n -eq 2 ]; then continue; fi; echo $(echo n=$n); done",
        ])
        .stdout();
    assert_eq!(out.trim(), "n=1\nn=3");
}

#[test]
fn rash_for_arithmetic_accumulator() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "sum=0; for n in 1 2 3; do sum=$((sum + n)); done; echo $sum",
        ])
        .stdout();
    assert_eq!(out.trim(), "6");
}

#[test]
fn rash_while_command_substitution_in_test() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "x=0; while [ $x -lt $(echo 3) ]; do echo $x; x=$((x + 1)); done",
        ])
        .stdout();
    assert_eq!(out.trim(), "0\n1\n2");
}

#[test]
fn rash_while_body_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "x=0; while [ $x -lt $(echo 2) ]; do x=$((x + 1)); echo $(echo step-$x); done",
        ])
        .stdout();
    assert_eq!(out.trim(), "step-1\nstep-2");
}

#[test]
fn rash_if_test_uses_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "if [ \"$(echo yes)\" = yes ]; then echo ok; fi"])
        .stdout();
    assert_eq!(out.trim(), "ok");
}

#[test]
fn rash_if_n_test_on_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "if [ -n \"$(echo text)\" ]; then echo nonempty; fi"])
        .stdout();
    assert_eq!(out.trim(), "nonempty");
}

#[test]
fn rash_case_word_from_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "case $(echo hi) in hi) echo matched ;; *) echo no ;; esac",
        ])
        .stdout();
    assert_eq!(out.trim(), "matched");
}

#[test]
fn rash_case_in_for_with_command_substitution_pattern() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "for f in one two; do case $f in $(echo one)) echo match ;; *) echo no ;; esac; done",
        ])
        .stdout();
    assert_eq!(out.trim(), "match\nno");
}

#[test]
fn rash_if_pipeline_in_condition() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "if echo hi | grep -q hi; then echo pipe-ok; fi"])
        .stdout();
    assert_eq!(out.trim(), "pipe-ok");
}

#[test]
fn rash_or_brace_with_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args(["-c", "false || { echo $(echo brace); }"])
        .stdout();
    assert_eq!(out.trim(), "brace");
}

#[test]
fn rash_function_for_loop_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "tags() { for x in 1 2; do echo $(echo tag-$x); done; }; tags",
        ])
        .stdout();
    assert_eq!(out.trim(), "tag-1\ntag-2");
}

#[test]
fn rash_for_redirect_and_read_back() {
    let dir = TestDir::new();
    let path = dir.join("items.txt");
    let status = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            &format!(
                "for d in one two; do echo $d >> {}; done; cat {}",
                path.display(),
                path.display()
            ),
        ])
        .status();
    assert_eq!(status, 0);
    assert_eq!(dir.read("items.txt").trim(), "one\ntwo");
}

#[test]
fn rash_for_case_with_arithmetic_expansion() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "for i in 1 2; do case $((i*2)) in 2|4) echo pair-$i ;; esac; done",
        ])
        .stdout();
    assert_eq!(out.trim(), "pair-1\npair-2");
}

#[test]
fn rash_for_runs_exported_subshell() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "export P=parent; for x in 1; do sh -c 'echo $P'; done",
        ])
        .stdout();
    assert_eq!(out.trim(), "parent");
}

#[test]
fn rash_while_arithmetic_loop() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "n=1; while [ $n -le 3 ]; do echo $((n*10)); n=$((n+1)); done",
        ])
        .stdout();
    assert_eq!(out.trim(), "10\n20\n30");
}

#[test]
fn rash_set_e_exits_inside_for_loop() {
    let status = Rustbox::new()
        .applet("rash")
        .args(["-c", "set -e; for x in ok; do false; done; echo never"])
        .status();
    assert_ne!(status, 0);
}

#[test]
fn rash_for_break_after_command_substitution() {
    let out = Rustbox::new()
        .applet("rash")
        .args([
            "-c",
            "for n in 1 2 3; do echo $(echo n=$n); if [ $n -eq 2 ]; then break; fi; done",
        ])
        .stdout();
    assert_eq!(out.trim(), "n=1\nn=2");
}
