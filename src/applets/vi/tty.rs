use super::editor::{Editor, Mode};
use std::io::{self, Write};

pub fn render(editor: &Editor, out: &mut impl Write) -> io::Result<()> {
    let rows = editor.rows as usize;
    let cols = editor.cols as usize;
    let text_rows = rows.saturating_sub(1).max(1);

    let scroll = editor.cursor().0.saturating_sub(text_rows / 2);
    let scroll = scroll.min(editor.lines().len().saturating_sub(1));

    write!(out, "\x1b[H\x1b[J")?;
    for display_row in 0..text_rows {
        let line_idx = scroll + display_row;
        if line_idx < editor.lines().len() {
            let line = &editor.lines()[line_idx];
            let end = line
                .char_indices()
                .nth(cols)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            writeln!(out, "\r{}", &line[..end])?;
        } else {
            writeln!(out, "\r~")?;
        }
    }

    let status_row = rows;
    if let Some((prefix, line)) = editor.status_prompt() {
        let mut status = String::with_capacity(prefix.len() + line.len());
        status.push_str(prefix);
        status.push_str(line);
        let end = status
            .char_indices()
            .nth(cols)
            .map(|(i, _)| i)
            .unwrap_or(status.len());
        write!(out, "\x1b[{status_row};1H\x1b[K{}", &status[..end])?;
    } else {
        let mode_label = match editor.mode() {
            Mode::Normal => "-- NORMAL --",
            Mode::Insert => "-- INSERT --",
            Mode::Replace => "-- REPLACE --",
            Mode::Command => "-- COMMAND --",
            Mode::SearchForward | Mode::SearchBackward => "-- NORMAL --",
        };
        write!(out, "\x1b[{status_row};1H\x1b[K{mode_label}")?;
    }

    let screen_row = editor.cursor().0.saturating_sub(scroll) + 1;
    let screen_col = editor.cursor().1 + 1;
    write!(
        out,
        "\x1b[{};{}H\x1b[?25h",
        screen_row.min(text_rows),
        screen_col.min(cols)
    )?;
    out.flush()
}

pub fn terminal_size() -> (u16, u16) {
    let mut ws = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) } == 0
        && ws.ws_row > 0
        && ws.ws_col > 0
    {
        (ws.ws_col, ws.ws_row)
    } else {
        (80, 24)
    }
}

pub struct RawMode {
    saved: rustix::termios::Termios,
}

impl RawMode {
    pub fn enable() -> io::Result<Self> {
        use rustix::termios::{tcgetattr, tcsetattr, OptionalActions};
        let stdin = rustix::stdio::stdin();
        let saved = tcgetattr(stdin).map_err(io::Error::from)?;
        let mut raw = saved.clone();
        raw.make_raw();
        tcsetattr(stdin, OptionalActions::Flush, &raw).map_err(io::Error::from)?;
        Ok(Self { saved })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        use rustix::termios::{tcsetattr, OptionalActions};
        let stdin = rustix::stdio::stdin();
        let _ = tcsetattr(stdin, OptionalActions::Flush, &self.saved);
        let mut out = io::stdout();
        let _ = write!(out, "\x1b[?25h\x1b[0m");
        let _ = out.flush();
    }
}

pub fn read_input(buf: &mut [u8]) -> io::Result<usize> {
    use rustix::io::read;
    use rustix::stdio::stdin;
    read(stdin(), buf).map_err(io::Error::from)
}
