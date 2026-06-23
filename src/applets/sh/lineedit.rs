use std::io::{self, IsTerminal, Write};

#[derive(Clone, Debug, Default)]
pub struct History {
    entries: Vec<String>,
    browse: Option<usize>,
    draft: Option<String>,
}

impl History {
    pub fn push(&mut self, line: &str) {
        let line = line.trim_end_matches(['\n', '\r']);
        if line.is_empty() {
            return;
        }
        if self.entries.last().is_some_and(|last| last == line) {
            return;
        }
        self.entries.push(line.to_string());
        self.browse = None;
        self.draft = None;
    }

    pub fn up(&mut self, current: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        if self.browse.is_none() {
            self.draft = Some(current.to_string());
            self.browse = Some(self.entries.len() - 1);
        } else if let Some(idx) = self.browse {
            if idx == 0 {
                return None;
            }
            self.browse = Some(idx - 1);
        }
        self.entries.get(self.browse?).cloned()
    }

    pub fn down(&mut self, _current: &str) -> Option<String> {
        let idx = self.browse?;
        if idx + 1 >= self.entries.len() {
            self.browse = None;
            return self.draft.take();
        }
        self.browse = Some(idx + 1);
        self.entries.get(self.browse?).cloned()
    }

    pub fn reset_browse(&mut self) {
        self.browse = None;
        self.draft = None;
    }

    pub fn load_file(path: &str) -> Self {
        let mut history = History::default();
        if let Ok(text) = std::fs::read_to_string(path) {
            for line in text.lines() {
                history.push(line);
            }
        }
        history
    }

    pub fn save_file(&self, path: &str) {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let mut out = String::new();
        for line in &self.entries {
            out.push_str(line);
            out.push('\n');
        }
        let _ = std::fs::write(path, out);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Up,
    Down,
    Interrupt,
    Eof,
    Unknown,
}

pub fn decode_key(bytes: &[u8]) -> (Key, usize) {
    if bytes.is_empty() {
        return (Key::Unknown, 0);
    }
    match bytes[0] {
        b'\r' | b'\n' => (Key::Enter, 1),
        0x04 => (Key::Eof, 1),
        0x03 => (Key::Interrupt, 1),
        0x7f | 0x08 => (Key::Backspace, 1),
        b'\t' => (Key::Char('\t'), 1),
        0x01 => (Key::Home, 1),
        0x05 => (Key::End, 1),
        0x1b => {
            if bytes.len() < 3 {
                return (Key::Unknown, 0);
            }
            if bytes[1] == b'[' {
                if bytes.len() == 2 {
                    return (Key::Unknown, 0);
                }
                return match bytes[2] {
                    b'A' => (Key::Up, 3),
                    b'B' => (Key::Down, 3),
                    b'C' => (Key::Right, 3),
                    b'D' => (Key::Left, 3),
                    b'H' => (Key::Home, 3),
                    b'F' => (Key::End, 3),
                    b'1' | b'3' | b'4' if bytes.len() < 4 => (Key::Unknown, 0),
                    b'1' if bytes[3] == b'~' => (Key::Home, 4),
                    b'3' if bytes[3] == b'~' => (Key::Delete, 4),
                    b'4' if bytes[3] == b'~' => (Key::End, 4),
                    _ if bytes.len() >= 6 && bytes[3] == b';' => match bytes[5] {
                        b'C' => (Key::Right, 6),
                        b'D' => (Key::Left, 6),
                        b'A' => (Key::Up, 6),
                        b'B' => (Key::Down, 6),
                        b'H' => (Key::Home, 6),
                        b'F' => (Key::End, 6),
                        _ => (Key::Unknown, 1),
                    },
                    _ => (Key::Unknown, 1),
                };
            }
            (Key::Unknown, 1)
        }
        b if b.is_ascii() && !b.is_ascii_control() => (Key::Char(b as char), 1),
        _ => (Key::Unknown, 1),
    }
}

struct LineState {
    text: String,
    cursor: usize,
}

impl LineState {
    fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    fn set_text(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
    }

    fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.text.remove(self.cursor);
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.text.len();
    }
}

pub fn read_line_editable(prompt: &str, history: &mut History) -> io::Result<Option<String>> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return read_line_plain(prompt);
    }
    read_line_tty(prompt, history)
}

fn read_line_plain(prompt: &str) -> io::Result<Option<String>> {
    let mut stdout = io::stdout();
    write!(stdout, "{prompt}")?;
    stdout.flush()?;
    let mut line = String::new();
    let n = io::stdin().read_line(&mut line)?;
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(line))
}

#[cfg(unix)]
fn read_line_tty(prompt: &str, history: &mut History) -> io::Result<Option<String>> {
    use rustix::fd::AsFd;
    use rustix::stdio;
    use rustix::termios::{tcgetattr, tcsetattr, OptionalActions};

    let stdin = stdio::stdin();
    let saved = tcgetattr(stdin.as_fd())?;
    let mut raw = saved.clone();
    raw.make_raw();
    tcsetattr(stdin.as_fd(), OptionalActions::Now, &raw)?;

    let result = read_line_tty_inner(prompt, history, stdin.as_fd());

    let _ = tcsetattr(stdin.as_fd(), OptionalActions::Now, &saved);
    result
}

#[cfg(not(unix))]
fn read_line_tty(prompt: &str, history: &mut History) -> io::Result<Option<String>> {
    read_line_plain(prompt)
}

#[cfg(unix)]
fn read_line_tty_inner(
    prompt: &str,
    history: &mut History,
    stdin: rustix::fd::BorrowedFd<'_>,
) -> io::Result<Option<String>> {
    use rustix::io::read;

    let mut stdout = io::stdout();
    let mut line = LineState::new();
    let mut buf = [0u8; 8];
    let mut pending = Vec::new();

    redraw(prompt, &line, &mut stdout)?;

    loop {
        let n = read(stdin, &mut buf).map_err(io::Error::from)?;
        if n == 0 {
            writeln!(stdout)?;
            stdout.flush()?;
            return Ok(None);
        }
        pending.extend_from_slice(&buf[..n]);

        while !pending.is_empty() {
            let (key, consumed) = decode_key(&pending);
            if consumed == 0 {
                break;
            }
            pending.drain(..consumed);

            match key {
                Key::Enter => {
                    writeln!(stdout)?;
                    stdout.flush()?;
                    history.reset_browse();
                    return Ok(Some(line.text));
                }
                Key::Eof if line.text.is_empty() => {
                    writeln!(stdout)?;
                    stdout.flush()?;
                    return Ok(None);
                }
                Key::Interrupt => {
                    line = LineState::new();
                    history.reset_browse();
                    writeln!(stdout)?;
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::Backspace => {
                    line.backspace();
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::Delete => {
                    line.delete();
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::Left => {
                    line.move_left();
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::Right => {
                    line.move_right();
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::Home => {
                    line.move_home();
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::End => {
                    line.move_end();
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::Up => {
                    if let Some(prev) = history.up(&line.text) {
                        line.set_text(prev);
                        redraw(prompt, &line, &mut stdout)?;
                    }
                }
                Key::Down => {
                    if let Some(next) = history.down(&line.text) {
                        line.set_text(next);
                        redraw(prompt, &line, &mut stdout)?;
                    }
                }
                Key::Char(ch) => {
                    line.insert(ch);
                    redraw(prompt, &line, &mut stdout)?;
                }
                Key::Eof | Key::Unknown => {}
            }
        }
    }
}

fn redraw(prompt: &str, line: &LineState, stdout: &mut io::Stdout) -> io::Result<()> {
    write!(stdout, "\r{prompt}{}\x1b[K", line.text)?;
    let tail = line.text.len().saturating_sub(line.cursor);
    if tail > 0 {
        write!(stdout, "\x1b[{tail}D")?;
    }
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_skips_empty_and_duplicates() {
        let mut h = History::default();
        h.push("ls");
        h.push("");
        h.push("ls");
        assert_eq!(h.entries, vec!["ls".to_string()]);
        h.push("pwd");
        assert_eq!(h.entries.len(), 2);
    }

    #[test]
    fn history_up_down() {
        let mut h = History::default();
        h.push("one");
        h.push("two");
        assert_eq!(h.up(""), Some("two".to_string()));
        assert_eq!(h.up(""), Some("one".to_string()));
        assert_eq!(h.down(""), Some("two".to_string()));
        assert_eq!(h.down(""), Some("".to_string()));
    }

    #[test]
    fn decodes_arrow_keys() {
        assert_eq!(decode_key(b"\x1b[A"), (Key::Up, 3));
        assert_eq!(decode_key(b"\x1b[B"), (Key::Down, 3));
        assert_eq!(decode_key(b"\x1b[C"), (Key::Right, 3));
        assert_eq!(decode_key(b"\x1b[D"), (Key::Left, 3));
    }
}
