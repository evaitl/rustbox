use crate::sys;
use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() {
        usage("sleep", "missing operand");
        return 1;
    }

    let seconds: f64 = match args[0].parse() {
        Ok(s) => s,
        Err(_) => {
            usage("sleep", "invalid time interval");
            return 1;
        }
    };

    if args.len() > 1 {
        usage("sleep", "extra operand");
        return 1;
    }

    match sys::sleep_seconds(seconds) {
        Ok(()) => 0,
        Err(e) => {
            usage("sleep", &e.to_string());
            1
        }
    }
}
