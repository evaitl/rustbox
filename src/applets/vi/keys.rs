#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Escape,
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Redraw,
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
        0x02 => (Key::PageUp, 1),
        0x06 => (Key::PageDown, 1),
        0x0c => (Key::Redraw, 1),
        0x7f | 0x08 => (Key::Backspace, 1),
        0x1b => {
            if bytes.len() == 1 {
                return (Key::Escape, 1);
            }
            if bytes[1] != b'[' {
                return (Key::Escape, 1);
            }
            if bytes.len() < 3 {
                return (Key::Unknown, 0);
            }
            match bytes[2] {
                b'A' => (Key::Up, 3),
                b'B' => (Key::Down, 3),
                b'C' => (Key::Right, 3),
                b'D' => (Key::Left, 3),
                b'H' => (Key::Home, 3),
                b'F' => (Key::End, 3),
                b'5' if bytes.len() < 4 => (Key::Unknown, 0),
                b'6' if bytes.len() < 4 => (Key::Unknown, 0),
                b'5' if bytes[3] == b'~' => (Key::PageUp, 4),
                b'6' if bytes[3] == b'~' => (Key::PageDown, 4),
                b'1' | b'3' | b'4' if bytes.len() < 4 => (Key::Unknown, 0),
                b'1' if bytes[3] == b'~' => (Key::Home, 4),
                b'3' if bytes[3] == b'~' => (Key::Delete, 4),
                b'4' if bytes[3] == b'~' => (Key::End, 4),
                _ => (Key::Unknown, 1),
            }
        }
        b if b.is_ascii() && !b.is_ascii_control() => (Key::Char(b as char), 1),
        _ => (Key::Unknown, 1),
    }
}

/// Expand a human-readable key script (`<Esc>`, `<Enter>`, …) into raw bytes.
pub fn expand_key_script(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = bytes[i + 1..].iter().position(|&b| b == b'>') {
                let token = std::str::from_utf8(&bytes[i + 1..i + 1 + end])
                    .unwrap_or("")
                    .trim();
                match token {
                    "Esc" | "ESC" => out.push(0x1b),
                    "Enter" | "CR" => out.push(b'\n'),
                    "Left" => out.extend_from_slice(b"\x1b[D"),
                    "Right" => out.extend_from_slice(b"\x1b[C"),
                    "Up" => out.extend_from_slice(b"\x1b[A"),
                    "Down" => out.extend_from_slice(b"\x1b[B"),
                    "Home" => out.extend_from_slice(b"\x1b[H"),
                    "End" => out.extend_from_slice(b"\x1b[F"),
                    "PageUp" | "PgUp" => out.extend_from_slice(b"\x1b[5~"),
                    "PageDown" | "PgDn" => out.extend_from_slice(b"\x1b[6~"),
                    "C-f" | "C-F" => out.push(0x06),
                    "C-b" | "C-B" => out.push(0x02),
                    "C-l" | "C-L" => out.push(0x0c),
                    "Backspace" | "BS" => out.push(0x7f),
                    "Delete" | "Del" => out.extend_from_slice(b"\x1b[3~"),
                    other if other.len() == 1 => out.push(other.as_bytes()[0]),
                    _ => {}
                }
                i += end + 2;
                continue;
            }
        }
        if bytes[i] == b'\n' || bytes[i] == b'\r' {
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_esc_and_enter() {
        assert_eq!(expand_key_script("i<Esc>"), b"i\x1b");
        assert_eq!(expand_key_script("<Enter>"), b"\n");
    }

    #[test]
    fn decodes_page_and_control_keys() {
        assert_eq!(decode_key(&[0x06]), (Key::PageDown, 1));
        assert_eq!(decode_key(&[0x02]), (Key::PageUp, 1));
        assert_eq!(decode_key(&[0x0c]), (Key::Redraw, 1));
        assert_eq!(decode_key(b"\x1b[5~"), (Key::PageUp, 4));
        assert_eq!(decode_key(b"\x1b[6~"), (Key::PageDown, 4));
        assert_eq!(expand_key_script("<C-b>"), b"\x02");
        assert_eq!(expand_key_script("<PageDown>"), b"\x1b[6~");
    }
}
