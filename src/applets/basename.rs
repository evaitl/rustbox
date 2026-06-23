use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    let mut suffix: Option<&str> = None;
    let mut paths: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-s" => {
                i += 1;
                if i >= args.len() {
                    usage("basename", "option requires an argument -- 's'");
                    return 1;
                }
                suffix = Some(args[i]);
            }
            "-a" => {}
            s if s.starts_with('-') => {
                usage("basename", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
        i += 1;
    }

    if paths.is_empty() {
        usage("basename", "missing operand");
        return 1;
    }

    if paths.len() == 2 && suffix.is_none() {
        suffix = Some(paths[1]);
        paths.truncate(1);
    }

    for path in paths {
        println!("{}", basename_of(path, suffix));
    }
    0
}

fn basename_of(path: &str, suffix: Option<&str>) -> String {
    let trimmed = path.trim_end_matches('/');
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if name.is_empty() {
        return "/".to_string();
    }
    match suffix {
        Some(sfx) => name.strip_suffix(sfx).unwrap_or(name).to_string(),
        None => name.to_string(),
    }
}
