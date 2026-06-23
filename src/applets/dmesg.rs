use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut clear = false;
    let mut raw = false;

    for arg in args {
        match *arg {
            "-c" | "--clear" => clear = true,
            "-r" => raw = true,
            s if s.starts_with('-') => {
                usage("dmesg", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("dmesg", &format!("unexpected operand -- '{s}'"));
                return 1;
            }
        }
    }

    let mut buf = vec![0u8; 256 * 1024];
    let n = match sys::read_kernel_log(&mut buf) {
        Ok(n) => n,
        Err(e) => {
            eprintln(format!("dmesg: {e}"));
            return 1;
        }
    };
    buf.truncate(n);

    if raw {
        use std::io::Write;
        let _ = std::io::stdout().write_all(&buf);
    } else {
        for line in buf.split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }
            let text = String::from_utf8_lossy(line);
            let trimmed = strip_syslog_prefix(text.trim_end());
            println!("{trimmed}");
        }
    }

    if clear {
        if let Err(e) = sys::clear_kernel_log() {
            eprintln(format!("dmesg: {e}"));
            return 1;
        }
    }
    0
}

fn strip_syslog_prefix(line: &str) -> &str {
    let Some(rest) = line.strip_prefix('<') else {
        return line;
    };
    let Some((_, after)) = rest.split_once('>') else {
        return line;
    };
    after
}
