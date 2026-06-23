pub fn run(args: &[&str]) -> i32 {
    let mut no_newline = false;
    let mut start = 0;

    for (i, arg) in args.iter().enumerate() {
        match *arg {
            "-n" => no_newline = true,
            _ => {
                start = i;
                break;
            }
        }
    }

    let rest = &args[start..];
    if rest.is_empty() {
        println!();
        return 0;
    }

    let output = rest.join(" ");
    if no_newline {
        print!("{output}");
    } else {
        println!("{output}");
    }
    0
}
