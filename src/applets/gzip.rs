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

#[cfg(feature = "fuzzing")]
pub fn fuzz_parse_args(input: &str) {
    let args: Vec<String> = input
        .split_whitespace()
        .take(32)
        .map(String::from)
        .collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let _ = run(&refs);
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_input(data: &[u8]) {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    const MAX: usize = 64 * 1024;
    let data = if data.len() > MAX { &data[..MAX] } else { data };

    let _ = catch_unwind(AssertUnwindSafe(|| {
        crate::compress::gzip::fuzz_input(data);
    }));

    if data.len() > 16 * 1024 {
        return;
    }

    let dir = std::env::temp_dir().join(format!("rustbox-gzip-fuzz-{}", std::process::id()));
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let plain = dir.join("in");
    let gz = dir.join("in.gz");
    if plain.to_str().is_none() || gz.to_str().is_none() {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let plain_s = plain.to_str().unwrap();
    let gz_s = gz.to_str().unwrap();
    let _ = std::fs::write(&plain, data);
    let _ = std::fs::write(&gz, data);
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = run(&["-dc", gz_s]);
        let _ = run(&["-c", plain_s]);
        let _ = run(&["-dk", gz_s]);
        let _ = run(&["-fk", plain_s]);
    }));
    let _ = std::fs::remove_dir_all(&dir);
}
