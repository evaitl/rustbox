pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() {
        for (_key, value) in std::env::vars() {
            println!("{value}");
        }
        return 0;
    }

    let mut status = 0;
    for name in args {
        match std::env::var(name) {
            Ok(value) => println!("{value}"),
            Err(_) => status = 1,
        }
    }
    status
}
