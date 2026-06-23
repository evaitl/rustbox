use crate::sys;
use crate::{eprintln, usage};

struct Options {
    mindepth: Option<usize>,
    maxdepth: Option<usize>,
    tests: Vec<Test>,
    paths: Vec<String>,
}

#[derive(Clone, Debug)]
enum Test {
    Name(String),
    Type(char),
}

pub fn run(args: &[&str]) -> i32 {
    let mut opts = Options {
        mindepth: None,
        maxdepth: None,
        tests: Vec::new(),
        paths: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-maxdepth" => {
                i += 1;
                if i >= args.len() {
                    usage("find", "missing argument to `-maxdepth'");
                    return 1;
                }
                opts.maxdepth = match args[i].parse() {
                    Ok(n) => Some(n),
                    Err(_) => {
                        usage("find", "invalid `-maxdepth' value");
                        return 1;
                    }
                };
            }
            "-mindepth" => {
                i += 1;
                if i >= args.len() {
                    usage("find", "missing argument to `-mindepth'");
                    return 1;
                }
                opts.mindepth = match args[i].parse() {
                    Ok(n) => Some(n),
                    Err(_) => {
                        usage("find", "invalid `-mindepth' value");
                        return 1;
                    }
                };
            }
            "-name" => {
                i += 1;
                if i >= args.len() {
                    usage("find", "missing argument to `-name'");
                    return 1;
                }
                opts.tests.push(Test::Name(args[i].to_string()));
            }
            "-type" => {
                i += 1;
                if i >= args.len() {
                    usage("find", "missing argument to `-type'");
                    return 1;
                }
                let ty = match args[i].chars().next() {
                    Some(c) => c,
                    None => {
                        usage("find", "missing argument to `-type'");
                        return 1;
                    }
                };
                if !matches!(ty, 'f' | 'd' | 'l') {
                    usage("find", &format!("unknown argument to `-type': {}", args[i]));
                    return 1;
                }
                opts.tests.push(Test::Type(ty));
            }
            s if s.starts_with('-') => {
                usage("find", &format!("unknown predicate `{s}'"));
                return 1;
            }
            s => opts.paths.push(s.to_string()),
        }
        i += 1;
    }

    if opts.paths.is_empty() {
        opts.paths.push(".".to_string());
    }

    let mut status = 0;
    for path in &opts.paths {
        if let Err(code) = walk(path, 0, &opts) {
            status = code;
        }
    }
    status
}

fn walk(path: &str, depth: usize, opts: &Options) -> Result<(), i32> {
    if let Some(max) = opts.maxdepth {
        if depth > max {
            return Ok(());
        }
    }

    let st = match sys::lstat(path) {
        Ok(st) => st,
        Err(e) => {
            eprintln(format!("find: '{path}': {e}"));
            return Err(1);
        }
    };
    let ft = rustix::fs::FileType::from_raw_mode(st.st_mode);

    let min_ok = opts.mindepth.is_none_or(|min| depth >= min);
    if min_ok && matches_tests(path, ft, opts) {
        println!("{path}");
    }

    if !ft.is_dir() {
        return Ok(());
    }
    if depth == opts.maxdepth.unwrap_or(usize::MAX) {
        return Ok(());
    }

    let entries = match sys::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln(format!("find: '{path}': {e}"));
            return Err(1);
        }
    };

    for entry in entries {
        let child = join_path(path, &entry.name);
        walk(&child, depth + 1, opts)?;
    }
    Ok(())
}

fn matches_tests(path: &str, ft: rustix::fs::FileType, opts: &Options) -> bool {
    if opts.tests.is_empty() {
        return true;
    }
    opts.tests.iter().all(|test| match test {
        Test::Name(glob) => fnmatch(file_name(path), glob),
        Test::Type(ty) => match ty {
            'f' => ft.is_file(),
            'd' => ft.is_dir(),
            'l' => ft.is_symlink(),
            _ => false,
        },
    })
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn fnmatch(name: &str, glob: &str) -> bool {
    fnmatch_impl(name.as_bytes(), glob.as_bytes())
}

fn fnmatch_impl(name: &[u8], glob: &[u8]) -> bool {
    match glob.first().copied() {
        None => name.is_empty(),
        Some(b'*') => {
            if glob.len() == 1 {
                return true;
            }
            let rest = &glob[1..];
            for i in 0..=name.len() {
                if fnmatch_impl(&name[i..], rest) {
                    return true;
                }
            }
            false
        }
        Some(b'?') => {
            if name.is_empty() {
                false
            } else {
                fnmatch_impl(&name[1..], &glob[1..])
            }
        }
        Some(ch) => {
            if name.first().copied() == Some(ch) {
                fnmatch_impl(&name[1..], &glob[1..])
            } else {
                false
            }
        }
    }
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
    fn fnmatch_star() {
        assert!(fnmatch("foo.txt", "*.txt"));
        assert!(!fnmatch("foo.md", "*.txt"));
    }
}
