use crate::sys;
use crate::{eprintln, usage};
use rustix::fs::{FileType, Mode};

pub fn run(args: &[&str]) -> i32 {
    let mut mode = Mode::from_raw_mode(0o666);
    let mut operands: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-m" => {
                i += 1;
                if i >= args.len() {
                    usage("mknod", "option requires an argument -- 'm'");
                    return 1;
                }
                match parse_mode(args[i]) {
                    Ok(m) => mode = m,
                    Err(()) => {
                        usage("mknod", &format!("invalid mode '{}'", args[i]));
                        return 1;
                    }
                }
            }
            s if s.starts_with("-m") => {
                let octal = &s[2..];
                if octal.is_empty() {
                    usage("mknod", "option requires an argument -- 'm'");
                    return 1;
                }
                match parse_mode(octal) {
                    Ok(m) => mode = m,
                    Err(()) => {
                        usage("mknod", &format!("invalid mode '{octal}'"));
                        return 1;
                    }
                }
            }
            s if s.starts_with('-') => {
                usage("mknod", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => operands.push(s),
        }
        i += 1;
    }

    if operands.len() < 2 {
        usage("mknod", "missing operand");
        return 1;
    }

    let name = operands[0];
    let node_type = operands[1];

    match node_type {
        "p" => {
            if operands.len() != 2 {
                usage("mknod", "extra operand");
                return 1;
            }
            match sys::mkfifo(name, mode) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln(format!("mknod: {name}: {e}"));
                    1
                }
            }
        }
        "b" | "c" | "u" => {
            if operands.len() != 4 {
                usage("mknod", "missing major/minor device numbers");
                return 1;
            }
            let file_type = if node_type == "b" {
                FileType::BlockDevice
            } else {
                FileType::CharacterDevice
            };
            let major = match operands[2].parse::<u32>() {
                Ok(n) => n,
                Err(_) => {
                    usage("mknod", "invalid major device number");
                    return 1;
                }
            };
            let minor = match operands[3].parse::<u32>() {
                Ok(n) => n,
                Err(_) => {
                    usage("mknod", "invalid minor device number");
                    return 1;
                }
            };
            match sys::mknod(name, file_type, mode, major, minor) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln(format!("mknod: {name}: {e}"));
                    1
                }
            }
        }
        _ => {
            usage("mknod", &format!("invalid device type '{node_type}'"));
            1
        }
    }
}

fn parse_mode(s: &str) -> Result<Mode, ()> {
    let value = u32::from_str_radix(s, 8).map_err(|_| ())?;
    if value > 0o7777 {
        return Err(());
    }
    Ok(Mode::from_raw_mode(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_octal_mode() {
        assert_eq!(parse_mode("644").unwrap(), Mode::from_raw_mode(0o644));
    }

    #[test]
    fn rejects_invalid_type_operand() {
        assert_eq!(run(&["/tmp/x", "q"]), 1);
    }
}
