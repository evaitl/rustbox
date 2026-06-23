use crate::{eprintln, usage};

#[cfg(target_os = "linux")]
use rustix::process;

pub fn run(args: &[&str]) -> i32 {
    #[cfg(not(target_os = "linux"))]
    {
        eprintln("pivot_root: not supported on this platform");
        return 1;
    }

    #[cfg(target_os = "linux")]
    {
        if args.len() != 2 {
            usage("pivot_root", "bad usage");
            return 1;
        }

        let new_root = args[0];
        let put_old = args[1];
        if new_root.is_empty() || put_old.is_empty() {
            usage("pivot_root", "bad usage");
            return 1;
        }

        if let Err(e) = process::pivot_root(new_root, put_old) {
            eprintln(format!(
                "pivot_root: failed to change root from '{new_root}' to '{put_old}': {e}"
            ));
            return 1;
        }
        0
    }
}
