mod editor;
mod keys;
mod tty;

use crate::{eprintln, usage};
use editor::{Editor, RunResult};
use keys::{decode_key, expand_key_script};
use std::fs;
use std::io::{self, IsTerminal};

pub fn run(args: &[&str]) -> i32 {
    let mut test_keys: Option<String> = None;
    let mut file: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-T" => {
                i += 1;
                if i >= args.len() {
                    usage("vi", "option requires an argument -- 'T'");
                    return 1;
                }
                test_keys = Some(args[i].to_string());
            }
            "-h" | "--help" => {
                usage(
                    "vi",
                    "usage: vi [-T KEYSCRIPT] FILE\n       vi FILE  (requires a VT100 terminal)",
                );
                return 0;
            }
            s if s.starts_with('-') => {
                usage("vi", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                if file.is_some() {
                    usage("vi", "too many arguments");
                    return 1;
                }
                file = Some(s.to_string());
            }
        }
        i += 1;
    }

    let path = match file {
        Some(p) => p,
        None => {
            usage("vi", "usage: vi [-T KEYSCRIPT] FILE");
            return 1;
        }
    };

    let mut editor = match Editor::open(&path) {
        Ok(e) => e,
        Err(code) => return code,
    };

    if let Some(script_path) = test_keys {
        return run_test_script(&mut editor, &script_path);
    }

    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        eprintln("vi: terminal required (use -T KEYSCRIPT for scripted input)");
        return 1;
    }

    run_interactive(&mut editor)
}

fn run_test_script(editor: &mut Editor, script_path: &str) -> i32 {
    let script = match fs::read_to_string(script_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln(format!("vi: {script_path}: {e}"));
            return 1;
        }
    };
    let bytes = expand_key_script(&script);
    run_bytes(editor, &bytes)
}

fn run_bytes(editor: &mut Editor, bytes: &[u8]) -> i32 {
    let mut pending = bytes.to_vec();
    while !pending.is_empty() {
        let (key, n) = decode_key(&pending);
        if n == 0 {
            break;
        }
        pending.drain(..n);
        if matches!(key, keys::Key::Eof) {
            break;
        }
        if matches!(key, keys::Key::Interrupt) {
            return 130;
        }
        match editor.handle_key(key) {
            RunResult::Continue => {}
            RunResult::Exit(code) => return code,
        }
    }
    eprintln("vi: not finished (missing :wq or :q!)");
    1
}

fn run_interactive(editor: &mut Editor) -> i32 {
    let (cols, rows) = tty::terminal_size();
    editor.set_size(rows, cols);

    let _raw = match tty::RawMode::enable() {
        Ok(r) => r,
        Err(e) => {
            eprintln(format!("vi: {e}"));
            return 1;
        }
    };

    let mut out = io::stdout();

    let mut pending = Vec::new();
    let mut buf = [0u8; 64];
    let mut last_rows = 0u16;
    let mut last_cols = 0u16;
    loop {
        let (cols, rows) = tty::terminal_size();
        if rows != last_rows || cols != last_cols {
            editor.set_size(rows, cols);
            last_rows = rows;
            last_cols = cols;
        }

        if let Err(e) = tty::render(editor, &mut out) {
            eprintln(format!("vi: {e}"));
            return 1;
        }

        let n = match tty::read_input(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                eprintln(format!("vi: {e}"));
                return 1;
            }
        };
        pending.extend_from_slice(&buf[..n]);

        while !pending.is_empty() {
            let (key, consumed) = decode_key(&pending);
            if consumed == 0 {
                break;
            }
            pending.drain(..consumed);
            if matches!(key, keys::Key::Eof) {
                return 0;
            }
            if matches!(key, keys::Key::Interrupt) {
                return 130;
            }
            match editor.handle_key(key) {
                RunResult::Continue => {}
                RunResult::Exit(code) => return code,
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::editor::Editor;
    use super::keys::expand_key_script;
    use super::run_bytes;
    use std::fs;

    #[test]
    fn insert_and_wq() {
        let dir = std::env::temp_dir().join(format!("rustbox-vi-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("file.txt");
        fs::write(&path, "hello").unwrap();
        let keys = dir.join("keys.txt");
        fs::write(&keys, "A world<Esc>\n:wq<Enter>\n").unwrap();

        let mut editor = Editor::open(path.to_str().unwrap()).unwrap();
        let script = fs::read_to_string(&keys).unwrap();
        let code = run_bytes(&mut editor, &expand_key_script(&script));
        assert_eq!(code, 0);
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
        let _ = fs::remove_dir_all(&dir);
    }
}
