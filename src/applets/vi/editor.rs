use super::keys::Key;
use std::fs;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Replace,
    Command,
    SearchForward,
    SearchBackward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResult {
    Continue,
    Exit(i32),
}

#[derive(Clone)]
struct Snapshot {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

enum Pending {
    None,
    G,
    R,
    D,
    Z,
    Y,
    C,
}

pub struct Editor {
    pub path: String,
    lines: Vec<String>,
    row: usize,
    col: usize,
    mode: Mode,
    cmd_line: String,
    dirty: bool,
    undo: Vec<Snapshot>,
    count: usize,
    pending: Pending,
    last_search: Option<String>,
    last_search_forward: bool,
    yank: String,
    yank_linewise: bool,
    pub rows: u16,
    pub cols: u16,
}

impl Editor {
    pub fn open(path: &str) -> Result<Self, i32> {
        let lines = if let Ok(text) = fs::read_to_string(path) {
            if text.is_empty() {
                vec![String::new()]
            } else {
                text.split_inclusive('\n')
                    .map(|line| line.strip_suffix('\n').unwrap_or(line).to_string())
                    .collect()
            }
        } else {
            vec![String::new()]
        };
        Ok(Self {
            path: path.to_string(),
            lines,
            row: 0,
            col: 0,
            mode: Mode::Normal,
            cmd_line: String::new(),
            dirty: false,
            undo: Vec::new(),
            count: 0,
            pending: Pending::None,
            last_search: None,
            last_search_forward: true,
            yank: String::new(),
            yank_linewise: false,
            rows: 24,
            cols: 80,
        })
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn set_size(&mut self, rows: u16, cols: u16) {
        self.rows = rows.max(2);
        self.cols = cols.max(20);
    }

    pub fn status_prompt(&self) -> Option<(&'static str, &str)> {
        match self.mode {
            Mode::Command => Some((":", &self.cmd_line)),
            Mode::SearchForward => Some(("/", &self.cmd_line)),
            Mode::SearchBackward => Some(("?", &self.cmd_line)),
            _ => None,
        }
    }

    pub fn handle_key(&mut self, key: Key) -> RunResult {
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Replace => self.handle_replace(key),
            Mode::Command => self.handle_command(key),
            Mode::SearchForward | Mode::SearchBackward => self.handle_search(key),
        }
    }

    fn take_count(&mut self) -> usize {
        let c = if self.count == 0 { 1 } else { self.count };
        self.count = 0;
        c
    }

    fn push_undo(&mut self) {
        self.undo.push(Snapshot {
            lines: self.lines.clone(),
            row: self.row,
            col: self.col,
        });
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }

    fn clamp_cursor(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.row = self.row.min(self.lines.len() - 1);
        let len = self.lines[self.row].len();
        self.col = self.col.min(len);
    }

    fn move_left(&mut self, n: usize) {
        for _ in 0..n {
            if self.col > 0 {
                self.col -= 1;
            } else if self.row > 0 {
                self.row -= 1;
                self.col = self.lines[self.row].len();
            }
        }
    }

    fn move_right(&mut self, n: usize) {
        for _ in 0..n {
            let len = self.lines[self.row].len();
            if self.col < len {
                self.col += 1;
            } else if self.row + 1 < self.lines.len() {
                self.row += 1;
                self.col = 0;
            }
        }
    }

    fn move_up(&mut self, n: usize) {
        self.row = self.row.saturating_sub(n);
        self.clamp_cursor();
    }

    fn move_down(&mut self, n: usize) {
        self.row = (self.row + n).min(self.lines.len() - 1);
        self.clamp_cursor();
    }

    fn text_page_lines(&self) -> usize {
        self.rows.saturating_sub(1).max(1) as usize
    }

    fn page_up(&mut self, pages: usize) {
        let step = self.text_page_lines().saturating_mul(pages);
        self.row = self.row.saturating_sub(step);
        self.clamp_cursor();
    }

    fn page_down(&mut self, pages: usize) {
        let step = self.text_page_lines().saturating_mul(pages);
        self.row = (self.row + step).min(self.lines.len() - 1);
        self.clamp_cursor();
    }

    fn move_line_start(&mut self) {
        self.col = 0;
    }

    fn move_line_end(&mut self) {
        let len = self.lines[self.row].len();
        self.col = if len == 0 { 0 } else { len - 1 };
    }

    fn move_first_nonblank(&mut self) {
        self.col = 0;
        let line = &self.lines[self.row];
        let off = line.find(|c: char| !c.is_whitespace()).unwrap_or(0);
        self.col = off.min(line.len());
    }

    fn go_to_line(&mut self, line_1based: usize) {
        if line_1based == 0 {
            self.row = 0;
        } else {
            self.row = line_1based.saturating_sub(1).min(self.lines.len() - 1);
        }
        self.clamp_cursor();
    }

    fn delete_char(&mut self, n: usize) {
        self.push_undo();
        for _ in 0..n {
            let line_len = self.lines[self.row].len();
            if self.col < line_len {
                self.lines[self.row].remove(self.col);
            } else if self.row + 1 < self.lines.len() {
                let next = self.lines.remove(self.row + 1);
                self.lines[self.row].push_str(&next);
            }
        }
        self.dirty = true;
        self.clamp_cursor();
    }

    fn delete_line(&mut self, n: usize) {
        if self.lines.is_empty() {
            return;
        }
        self.push_undo();
        let start = self.row;
        let end = (self.row + n).min(self.lines.len());
        self.lines.drain(start..end);
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.row = start.min(self.lines.len() - 1);
        self.dirty = true;
        self.clamp_cursor();
    }

    fn delete_word(&mut self, n: usize) {
        self.push_undo();
        for _ in 0..n {
            self.delete_word_once();
        }
        self.dirty = true;
        self.clamp_cursor();
    }

    fn delete_word_once(&mut self) {
        let end = self.word_span_end(self.row, self.col);
        if end > 0 {
            self.lines[self.row].drain(self.col..self.col + end);
        } else if self.col >= self.lines[self.row].len() && self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    fn word_span_end(&self, row: usize, col: usize) -> usize {
        let line = &self.lines[row];
        let line_len = line.len();
        if col >= line_len {
            return 0;
        }
        let rest = &line[col..];
        let mut end = 0usize;
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            end = c.len_utf8();
            if c.is_alphanumeric() {
                for c in chars.by_ref() {
                    if !c.is_alphanumeric() {
                        break;
                    }
                    end += c.len_utf8();
                }
            }
        }
        end
    }

    fn extract_words(&self, row: usize, col: usize, n: usize) -> (String, usize, usize) {
        let mut out = String::new();
        let mut col = col;
        for _ in 0..n {
            if row >= self.lines.len() {
                break;
            }
            let end = self.word_span_end(row, col);
            if end == 0 {
                break;
            }
            out.push_str(&self.lines[row][col..col + end]);
            col += end;
        }
        (out, row, col)
    }

    fn first_nonblank_col(line: &str) -> usize {
        line.find(|c: char| !c.is_whitespace()).unwrap_or(0)
    }

    fn delete_to_end_of_line(&mut self) {
        self.push_undo();
        let line_len = self.lines[self.row].len();
        if self.col < line_len {
            self.lines[self.row].truncate(self.col);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
        self.dirty = true;
        self.clamp_cursor();
    }

    fn delete_to_first_nonblank(&mut self) {
        self.push_undo();
        let first = Self::first_nonblank_col(&self.lines[self.row]);
        if self.col > first {
            self.lines[self.row].drain(first..self.col);
            self.col = first;
        }
        self.dirty = true;
        self.clamp_cursor();
    }

    fn yank_lines(&mut self, n: usize) {
        let end = (self.row + n).min(self.lines.len());
        self.yank = self.lines[self.row..end].join("\n");
        self.yank_linewise = true;
    }

    fn yank_word(&mut self, n: usize) {
        let (text, _, _) = self.extract_words(self.row, self.col, n);
        self.yank = text;
        self.yank_linewise = false;
    }

    fn yank_to_end_of_line(&mut self) {
        self.yank = self.lines[self.row][self.col..].to_string();
        self.yank_linewise = false;
    }

    fn change_line(&mut self, n: usize) {
        self.push_undo();
        let end = (self.row + n).min(self.lines.len());
        for line in &mut self.lines[self.row..end] {
            line.clear();
        }
        self.col = 0;
        self.mode = Mode::Insert;
        self.dirty = true;
    }

    fn change_word(&mut self, n: usize) {
        self.push_undo();
        for _ in 0..n {
            self.delete_word_once();
        }
        self.mode = Mode::Insert;
        self.dirty = true;
    }

    fn join_line(&mut self) {
        if self.row + 1 >= self.lines.len() {
            return;
        }
        self.push_undo();
        let line = &self.lines[self.row];
        let needs_space =
            !line.is_empty() && !line.ends_with(' ') && !self.lines[self.row + 1].starts_with(' ');
        let next = self.lines.remove(self.row + 1);
        if needs_space {
            self.lines[self.row].push(' ');
        }
        self.lines[self.row].push_str(&next);
        self.dirty = true;
        self.clamp_cursor();
    }

    fn paste_after(&mut self) {
        if self.yank.is_empty() {
            return;
        }
        self.push_undo();
        if self.yank_linewise {
            let new_lines: Vec<String> = self.yank.lines().map(str::to_string).collect();
            let insert_at = self.row + 1;
            for (i, line) in new_lines.iter().enumerate() {
                self.lines.insert(insert_at + i, line.clone());
            }
            self.row += 1;
            self.col = 0;
        } else {
            let insert_at = if self.col < self.lines[self.row].len() {
                self.col
                    + self.lines[self.row][self.col..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(1)
            } else {
                self.lines[self.row].len()
            };
            self.lines[self.row].insert_str(insert_at, &self.yank);
            self.col = insert_at + self.yank.len();
        }
        self.dirty = true;
        self.clamp_cursor();
    }

    fn paste_before(&mut self) {
        if self.yank.is_empty() {
            return;
        }
        self.push_undo();
        if self.yank_linewise {
            let new_lines: Vec<String> = self.yank.lines().map(str::to_string).collect();
            let insert_at = self.row;
            for (i, line) in new_lines.iter().enumerate() {
                self.lines.insert(insert_at + i, line.clone());
            }
            self.col = 0;
        } else {
            self.lines[self.row].insert_str(self.col, &self.yank);
            self.col += self.yank.len();
        }
        self.dirty = true;
        self.clamp_cursor();
    }

    fn insert_char(&mut self, ch: char) {
        self.lines[self.row].insert(self.col, ch);
        self.col += ch.len_utf8();
        self.dirty = true;
    }

    fn open_line_below(&mut self) {
        self.push_undo();
        self.lines.insert(self.row + 1, String::new());
        self.row += 1;
        self.col = 0;
        self.mode = Mode::Insert;
        self.dirty = true;
    }

    fn open_line_above(&mut self) {
        self.push_undo();
        self.lines.insert(self.row, String::new());
        self.col = 0;
        self.mode = Mode::Insert;
        self.dirty = true;
    }

    fn enter_insert(&mut self, col: usize) {
        self.push_undo();
        self.col = col.min(self.lines[self.row].len());
        self.mode = Mode::Insert;
    }

    fn all_matches(&self, pattern: &str) -> Vec<(usize, usize)> {
        if pattern.is_empty() {
            return Vec::new();
        }
        let mut matches = Vec::new();
        for (row, line) in self.lines.iter().enumerate() {
            let mut start = 0;
            while start < line.len() {
                let Some(off) = line[start..].find(pattern) else {
                    break;
                };
                let col = start + off;
                matches.push((row, col));
                start = col + 1;
            }
        }
        matches
    }

    fn pos_after((r, c): (usize, usize), (ar, ac): (usize, usize)) -> bool {
        r > ar || (r == ar && c > ac)
    }

    fn pos_before((r, c): (usize, usize), (br, bc): (usize, usize)) -> bool {
        r < br || (r == br && c < bc)
    }

    fn search_forward(&mut self, pattern: &str, repeat: bool) -> bool {
        let matches = self.all_matches(pattern);
        if matches.is_empty() {
            return false;
        }
        let anchor = (self.row, self.col);
        let next = if repeat {
            matches
                .iter()
                .find(|&&(r, c)| Self::pos_after((r, c), anchor))
                .or_else(|| matches.first())
        } else {
            matches
                .iter()
                .find(|&&(r, c)| r > anchor.0 || (r == anchor.0 && c >= anchor.1))
                .or_else(|| matches.first())
        };
        if let Some(&(row, col)) = next {
            self.row = row;
            self.col = col;
            self.clamp_cursor();
            true
        } else {
            false
        }
    }

    fn search_backward(&mut self, pattern: &str, repeat: bool) -> bool {
        let matches = self.all_matches(pattern);
        if matches.is_empty() {
            return false;
        }
        let anchor = (self.row, self.col);
        let prev = if repeat {
            matches
                .iter()
                .rev()
                .find(|&&(r, c)| Self::pos_before((r, c), anchor))
                .or_else(|| matches.last())
        } else {
            matches
                .iter()
                .rev()
                .find(|&&(r, c)| r < anchor.0 || (r == anchor.0 && c <= anchor.1))
                .or_else(|| matches.last())
        };
        if let Some(&(row, col)) = prev {
            self.row = row;
            self.col = col;
            self.clamp_cursor();
            true
        } else {
            false
        }
    }

    fn remember_search(&mut self, pattern: &str, forward: bool) {
        self.last_search = Some(pattern.to_string());
        self.last_search_forward = forward;
    }

    fn repeat_search(&mut self, forward: bool, count: usize) {
        let Some(pattern) = self.last_search.clone() else {
            return;
        };
        for _ in 0..count {
            let ok = if forward {
                self.search_forward(&pattern, true)
            } else {
                self.search_backward(&pattern, true)
            };
            if !ok {
                break;
            }
        }
    }

    fn run_search(&mut self, forward: bool) {
        let pattern = if self.cmd_line.is_empty() {
            self.last_search.clone().unwrap_or_default()
        } else {
            self.cmd_line.clone()
        };
        if pattern.is_empty() {
            return;
        }
        let found = if forward {
            self.search_forward(&pattern, false)
        } else {
            self.search_backward(&pattern, false)
        };
        if found {
            self.remember_search(&pattern, forward);
        }
    }

    fn handle_normal(&mut self, key: Key) -> RunResult {
        match key {
            Key::Char('0') if self.count == 0 => {
                self.move_line_start();
                self.pending = Pending::None;
            }
            Key::Char(c @ '1'..='9') => {
                self.count = self.count.saturating_mul(10) + (c as u8 - b'0') as usize;
            }
            Key::Char('h') | Key::Left => {
                let n = self.take_count();
                self.move_left(n);
                self.pending = Pending::None;
            }
            Key::Char('l') | Key::Right => {
                let n = self.take_count();
                self.move_right(n);
                self.pending = Pending::None;
            }
            Key::Char('j') | Key::Down => {
                let n = self.take_count();
                self.move_down(n);
                self.pending = Pending::None;
            }
            Key::Char('k') | Key::Up => {
                let n = self.take_count();
                self.move_up(n);
                self.pending = Pending::None;
            }
            Key::PageUp => {
                let n = self.take_count();
                self.page_up(n);
                self.pending = Pending::None;
            }
            Key::PageDown => {
                let n = self.take_count();
                self.page_down(n);
                self.pending = Pending::None;
            }
            Key::Redraw => {
                self.count = 0;
                self.pending = Pending::None;
            }
            Key::Char('$') | Key::End if matches!(self.pending, Pending::D) => {
                self.pending = Pending::None;
                self.take_count();
                self.delete_to_end_of_line();
            }
            Key::Char('$') | Key::End if matches!(self.pending, Pending::Y) => {
                self.pending = Pending::None;
                self.take_count();
                self.yank_to_end_of_line();
            }
            Key::Char('^') | Key::Home if matches!(self.pending, Pending::D) => {
                self.pending = Pending::None;
                self.take_count();
                self.delete_to_first_nonblank();
            }
            Key::Char('$') | Key::End => {
                self.move_line_end();
                self.count = 0;
                self.pending = Pending::None;
            }
            Key::Char('^') | Key::Home => {
                self.move_first_nonblank();
                self.count = 0;
                self.pending = Pending::None;
            }
            Key::Char('i') => {
                self.count = 0;
                self.enter_insert(self.col);
            }
            Key::Char('I') => {
                self.count = 0;
                self.move_line_start();
                self.enter_insert(0);
            }
            Key::Char('a') => {
                self.count = 0;
                self.push_undo();
                if self.col < self.lines[self.row].len() {
                    self.col += 1;
                }
                self.mode = Mode::Insert;
            }
            Key::Char('A') => {
                self.count = 0;
                self.push_undo();
                self.col = self.lines[self.row].len();
                self.mode = Mode::Insert;
            }
            Key::Char('o') => {
                self.count = 0;
                self.open_line_below();
            }
            Key::Char('O') => {
                self.count = 0;
                self.open_line_above();
            }
            Key::Char('x') => {
                let n = self.take_count();
                self.delete_char(n);
                self.pending = Pending::None;
            }
            Key::Char(c) if matches!(self.pending, Pending::R) => {
                self.pending = Pending::None;
                let n = self.take_count().max(1);
                self.push_undo();
                for _ in 0..n {
                    if self.col < self.lines[self.row].len() {
                        let clen = self.lines[self.row][self.col..]
                            .chars()
                            .next()
                            .map(char::len_utf8)
                            .unwrap_or(1);
                        self.lines[self.row]
                            .replace_range(self.col..self.col + clen, &c.to_string());
                        self.dirty = true;
                    }
                    self.move_right(1);
                }
            }
            Key::Char('d') => match self.pending {
                Pending::D => {
                    self.pending = Pending::None;
                    let n = self.take_count();
                    self.delete_line(n);
                }
                _ => {
                    self.pending = Pending::D;
                }
            },
            Key::Char('w') if matches!(self.pending, Pending::D) => {
                self.pending = Pending::None;
                let n = self.take_count();
                self.delete_word(n);
            }
            Key::Char('w') if matches!(self.pending, Pending::Y) => {
                self.pending = Pending::None;
                let n = self.take_count();
                self.yank_word(n);
            }
            Key::Char('w') if matches!(self.pending, Pending::C) => {
                self.pending = Pending::None;
                let n = self.take_count();
                self.change_word(n);
            }
            Key::Char('y') => match self.pending {
                Pending::Y => {
                    self.pending = Pending::None;
                    let n = self.take_count();
                    self.yank_lines(n);
                }
                _ => {
                    self.pending = Pending::Y;
                }
            },
            Key::Char('Y') => {
                let n = self.take_count();
                self.pending = Pending::None;
                self.yank_lines(n);
            }
            Key::Char('c') => match self.pending {
                Pending::C => {
                    self.pending = Pending::None;
                    let n = self.take_count();
                    self.change_line(n);
                }
                _ => {
                    self.pending = Pending::C;
                }
            },
            Key::Char('J') => {
                let n = self.take_count();
                self.pending = Pending::None;
                for _ in 0..n {
                    self.join_line();
                }
            }
            Key::Char('p') => {
                let n = self.take_count();
                self.pending = Pending::None;
                for _ in 0..n {
                    self.paste_after();
                }
            }
            Key::Char('P') => {
                let n = self.take_count();
                self.pending = Pending::None;
                for _ in 0..n {
                    self.paste_before();
                }
            }
            Key::Char('g') => match self.pending {
                Pending::G => {
                    self.go_to_line(1);
                    self.pending = Pending::None;
                    self.count = 0;
                }
                _ => {
                    self.pending = Pending::G;
                    self.count = 0;
                }
            },
            Key::Char('G') => {
                let n = self.take_count();
                if n <= 1 {
                    self.row = self.lines.len().saturating_sub(1);
                } else {
                    self.go_to_line(n);
                }
                self.clamp_cursor();
                self.pending = Pending::None;
            }
            Key::Char('r') => {
                self.pending = Pending::R;
                self.count = 0;
            }
            Key::Char('R') => {
                self.count = 0;
                self.pending = Pending::None;
                self.push_undo();
                self.mode = Mode::Replace;
            }
            Key::Char('u') => {
                self.count = 0;
                self.pending = Pending::None;
                if let Some(snap) = self.undo.pop() {
                    self.lines = snap.lines;
                    self.row = snap.row;
                    self.col = snap.col;
                    self.dirty = true;
                    self.clamp_cursor();
                }
            }
            Key::Char(':') => {
                self.count = 0;
                self.pending = Pending::None;
                self.mode = Mode::Command;
                self.cmd_line.clear();
            }
            Key::Char('/') => {
                self.count = 0;
                self.pending = Pending::None;
                self.mode = Mode::SearchForward;
                self.cmd_line.clear();
            }
            Key::Char('?') => {
                self.count = 0;
                self.pending = Pending::None;
                self.mode = Mode::SearchBackward;
                self.cmd_line.clear();
            }
            Key::Char('n') => {
                let n = self.take_count();
                self.pending = Pending::None;
                self.repeat_search(self.last_search_forward, n);
            }
            Key::Char('N') => {
                let n = self.take_count();
                self.pending = Pending::None;
                self.repeat_search(!self.last_search_forward, n);
            }
            Key::Char('Z') => match self.pending {
                Pending::Z => {
                    self.pending = Pending::None;
                    self.count = 0;
                    return match self.write_file() {
                        Ok(()) => RunResult::Exit(0),
                        Err(_) => RunResult::Exit(1),
                    };
                }
                _ => {
                    self.pending = Pending::Z;
                    self.count = 0;
                }
            },
            Key::Char('Q') if matches!(self.pending, Pending::Z) => {
                self.pending = Pending::None;
                self.count = 0;
                return RunResult::Exit(0);
            }
            Key::Escape => {
                self.count = 0;
                self.pending = Pending::None;
            }
            _ => {
                self.count = 0;
                self.pending = Pending::None;
            }
        }
        RunResult::Continue
    }

    fn handle_insert(&mut self, key: Key) -> RunResult {
        match key {
            Key::Escape => {
                if self.col > 0 && self.col == self.lines[self.row].len() {
                    self.col -= 1;
                }
                self.mode = Mode::Normal;
            }
            Key::Enter | Key::Char('\n') => {
                let rest = self.lines[self.row].split_off(self.col);
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
                self.col = 0;
                self.dirty = true;
            }
            Key::Backspace => {
                if self.col > 0 {
                    let line = &mut self.lines[self.row];
                    let prev = line[..self.col]
                        .chars()
                        .last()
                        .map(char::len_utf8)
                        .unwrap_or(1);
                    line.remove(self.col - prev);
                    self.col -= prev;
                    self.dirty = true;
                } else if self.row > 0 {
                    let rest = self.lines.remove(self.row);
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                    self.lines[self.row].push_str(&rest);
                    self.dirty = true;
                }
            }
            Key::Char(ch) => {
                self.insert_char(ch);
            }
            _ => {}
        }
        RunResult::Continue
    }

    fn handle_replace(&mut self, key: Key) -> RunResult {
        match key {
            Key::Escape => {
                self.mode = Mode::Normal;
            }
            Key::Enter => {
                self.handle_insert(key);
                self.mode = Mode::Replace;
            }
            Key::Char(ch) => {
                if self.col < self.lines[self.row].len() {
                    let start = self.col;
                    let clen = self.lines[self.row][start..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(1);
                    if start + clen <= self.lines[self.row].len() {
                        self.lines[self.row].replace_range(start..start + clen, &ch.to_string());
                        self.dirty = true;
                    }
                    self.move_right(1);
                } else if ch == '\n' {
                    self.handle_insert(Key::Enter);
                    self.mode = Mode::Replace;
                }
            }
            _ => {}
        }
        RunResult::Continue
    }

    fn handle_command(&mut self, key: Key) -> RunResult {
        match key {
            Key::Escape => {
                self.mode = Mode::Normal;
                self.cmd_line.clear();
            }
            Key::Enter => {
                let cmd = self.cmd_line.trim().to_string();
                self.cmd_line.clear();
                self.mode = Mode::Normal;
                return self.exec_command(&cmd);
            }
            Key::Backspace => {
                self.cmd_line.pop();
            }
            Key::Char(ch) => {
                self.cmd_line.push(ch);
            }
            _ => {}
        }
        RunResult::Continue
    }

    fn handle_search(&mut self, key: Key) -> RunResult {
        let forward = self.mode == Mode::SearchForward;
        match key {
            Key::Escape => {
                self.mode = Mode::Normal;
                self.cmd_line.clear();
            }
            Key::Enter => {
                self.mode = Mode::Normal;
                self.run_search(forward);
                self.cmd_line.clear();
            }
            Key::Backspace => {
                self.cmd_line.pop();
            }
            Key::Char(ch) => {
                self.cmd_line.push(ch);
            }
            _ => {}
        }
        RunResult::Continue
    }

    fn exec_command(&mut self, cmd: &str) -> RunResult {
        match cmd {
            "w" | "write" => match self.write_file() {
                Ok(()) => RunResult::Continue,
                Err(_) => RunResult::Exit(1),
            },
            "q" | "quit" => {
                if self.dirty {
                    RunResult::Continue
                } else {
                    RunResult::Exit(0)
                }
            }
            "q!" | "quit!" => RunResult::Exit(0),
            "wq" | "x" => match self.write_file() {
                Ok(()) => RunResult::Exit(0),
                Err(_) => RunResult::Exit(1),
            },
            "" => RunResult::Continue,
            _ => RunResult::Continue,
        }
    }

    pub fn write_file(&mut self) -> io::Result<()> {
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            out.push_str(line);
            if i + 1 < self.lines.len() {
                out.push('\n');
            }
        }
        fs::write(&self.path, out)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;
    use crate::applets::vi::keys::{decode_key, expand_key_script};

    fn keys(script: &str) -> Vec<Key> {
        let bytes = expand_key_script(script);
        let mut out = Vec::new();
        let mut pending = bytes.as_slice();
        while !pending.is_empty() {
            let (key, n) = decode_key(pending);
            if n == 0 {
                break;
            }
            pending = &pending[n..];
            out.push(key);
        }
        out
    }

    fn run_keys(editor: &mut Editor, script: &str) {
        for key in keys(script) {
            editor.handle_key(key);
        }
    }

    #[test]
    fn search_forward_finds_next_match() {
        let mut e = Editor::open("/dev/null").unwrap();
        e.lines = vec!["hello world".to_string()];
        run_keys(&mut e, "/world<Enter>");
        assert_eq!(e.cursor(), (0, 6));
    }

    #[test]
    fn search_n_repeats_forward() {
        let mut e = Editor::open("/dev/null").unwrap();
        e.lines = vec!["hello hello".to_string()];
        run_keys(&mut e, "/hello<Enter>n");
        assert_eq!(e.cursor(), (0, 6));
    }

    #[test]
    fn search_n_reverses_direction() {
        let mut e = Editor::open("/dev/null").unwrap();
        e.lines = vec!["foo bar foo".to_string()];
        run_keys(&mut e, "G$?foo<Enter>N");
        assert_eq!(e.cursor(), (0, 0));
    }

    #[test]
    fn page_down_moves_by_screen() {
        let mut e = Editor::open("/dev/null").unwrap();
        e.set_size(24, 80);
        e.lines = (0..30).map(|i| format!("line{i}")).collect();
        run_keys(&mut e, "<C-f>");
        assert_eq!(e.cursor().0, 23);
    }

    #[test]
    fn page_up_moves_by_screen() {
        let mut e = Editor::open("/dev/null").unwrap();
        e.set_size(24, 80);
        e.lines = (0..30).map(|i| format!("line{i}")).collect();
        e.row = 29;
        run_keys(&mut e, "<C-b>");
        assert_eq!(e.cursor().0, 6);
    }
}
