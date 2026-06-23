use crate::sys;
use crate::{eprintln, usage};
use rustix::fs::Mode;

pub fn run(args: &[&str]) -> i32 {
    let mut recursive = false;
    let mut mode_str: Option<&str> = None;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-R" | "--recursive" => recursive = true,
            s if s.starts_with('-') => {
                usage("chmod", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s if mode_str.is_none() => mode_str = Some(s),
            s => paths.push(s),
        }
    }

    let Some(mode_str) = mode_str else {
        usage("chmod", "missing operand");
        return 1;
    };
    if paths.is_empty() {
        usage("chmod", "missing operand after mode");
        return 1;
    }

    let mode = match parse_mode(mode_str) {
        Ok(mode) => mode,
        Err(()) => {
            usage("chmod", &format!("invalid mode: '{mode_str}'"));
            return 1;
        }
    };

    for path in paths {
        if let Err(e) = apply_mode(path, mode, recursive) {
            eprintln(format!("chmod: cannot access '{path}': {e}"));
            return 1;
        }
    }
    0
}

fn apply_mode(path: &str, mode: Mode, recursive: bool) -> sys::Result<()> {
    sys::chmod_path(path, mode)?;
    if recursive && sys::is_directory(path) {
        for entry in sys::read_dir(path)? {
            let child = join_path(path, &entry.name);
            apply_mode(&child, mode, true)?;
        }
    }
    Ok(())
}

fn parse_mode(s: &str) -> Result<Mode, ()> {
    let value = u32::from_str_radix(s, 8).map_err(|_| ())?;
    if value > 0o7777 {
        return Err(());
    }
    Ok(Mode::from_raw_mode(value))
}

fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_octal_mode() {
        assert_eq!(parse_mode("755").unwrap(), Mode::from_raw_mode(0o755));
        assert_eq!(parse_mode("0644").unwrap(), Mode::from_raw_mode(0o644));
    }
}
