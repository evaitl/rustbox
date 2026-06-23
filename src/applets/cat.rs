use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() {
        if sys::copy_fd_to_stdout(stdio::stdin()).is_err() {
            usage("cat", "write error");
            return 1;
        }
        return 0;
    }

    for path in args {
        match sys::open_read(path) {
            Ok(fd) => {
                if sys::copy_fd_to_stdout(fd).is_err() {
                    usage("cat", "write error");
                    return 1;
                }
            }
            Err(e) => {
                eprintln(format!("cat: {path}: {e}"));
                return 1;
            }
        }
    }
    0
}
