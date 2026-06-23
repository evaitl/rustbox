use crate::sys;
use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    if let Some(arg) = args.first() {
        if arg.starts_with('-') {
            usage("ps", &format!("invalid option -- '{arg}'"));
            return 1;
        }
        usage("ps", "extra operand");
        return 1;
    }

    match sys::list_processes() {
        Ok(procs) => {
            println!("  PID COMMAND");
            for proc in procs {
                println!("{:5} {}", proc.pid, proc.comm);
            }
            0
        }
        Err(e) => {
            usage("ps", &e.to_string());
            1
        }
    }
}
