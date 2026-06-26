use crate::compress::gzip;
use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut decompress = false;
    let mut stdout_mode = false;
    let mut force = false;
    let mut keep = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        if *arg == "--" {
            continue;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            if *arg == "-h" || *arg == "--help" {
                usage("gzip", "usage: gzip [-cdfk] [FILE]...");
                return 0;
            }
            for ch in arg.chars().skip(1) {
                match ch {
                    'c' => stdout_mode = true,
                    'd' => decompress = true,
                    'f' => force = true,
                    'k' => keep = true,
                    _ => {
                        usage("gzip", &format!("invalid option -- '{ch}'"));
                        return 1;
                    }
                }
            }
            continue;
        }
        paths.push(arg);
    }

    if paths.is_empty() {
        return if decompress {
            match gzip::decompress_stdin_to_stdout() {
                Ok(()) => 0,
                Err(e) => {
                    eprintln(format!("gzip: {e}"));
                    1
                }
            }
        } else {
            match gzip::compress_stdin_to_stdout() {
                Ok(()) => 0,
                Err(e) => {
                    eprintln(format!("gzip: {e}"));
                    1
                }
            }
        };
    }

    for path in paths {
        if let Err(code) = process_one(path, decompress, stdout_mode, force, keep) {
            return code;
        }
    }
    0
}

fn process_one(
    path: &str,
    decompress: bool,
    stdout_mode: bool,
    force: bool,
    keep: bool,
) -> Result<(), i32> {
    if decompress {
        let dst = if path.ends_with(".gz") {
            path.strip_suffix(".gz").unwrap_or(path).to_string()
        } else {
            format!("{path}.out")
        };
        if stdout_mode {
            return gzip::decompress_file_to_stdout(path).map_err(|e| {
                eprintln(format!("gzip: {path}: {e}"));
                1
            });
        }
        if sys::exists(&dst) && !force {
            eprintln(format!("gzip: {dst}: file already exists"));
            return Err(1);
        }
        gzip::decompress_file_to_path(path, &dst).map_err(|e| {
            eprintln(format!("gzip: {path}: {e}"));
            1
        })?;
        if !keep {
            let _ = sys::remove_file(path);
        }
        return Ok(());
    }

    let dst = format!("{path}.gz");
    if stdout_mode {
        return gzip::compress_file_to_stdout(path).map_err(|e| {
            eprintln(format!("gzip: {path}: {e}"));
            1
        });
    }
    if sys::exists(&dst) && !force {
        eprintln(format!("gzip: {dst}: file already exists"));
        return Err(1);
    }
    gzip::compress_file_to_path(path, &dst).map_err(|e| {
        eprintln(format!("gzip: {path}: {e}"));
        1
    })?;
    if !keep {
        let _ = sys::remove_file(path);
    }
    Ok(())
}
