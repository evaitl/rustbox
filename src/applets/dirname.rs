use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() {
        usage("dirname", "missing operand");
        return 1;
    }

    for arg in args {
        if arg.starts_with('-') {
            usage("dirname", &format!("invalid option -- '{arg}'"));
            return 1;
        }
        println!("{}", dirname_of(arg));
    }
    0
}

fn dirname_of(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }
    let trimmed = path.trim_end_matches('/');
    if !trimmed.contains('/') {
        return ".".to_string();
    }
    match trimmed.rfind('/') {
        None => ".".to_string(),
        Some(0) => "/".to_string(),
        Some(idx) => trimmed[..idx].to_string(),
    }
}
