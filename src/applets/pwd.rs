use crate::sys;
use crate::{eprintln, usage};

pub fn run(_args: &[&str]) -> i32 {
    match sys::current_dir() {
        Ok(path) => {
            println!("{path}");
            0
        }
        Err(e) => {
            usage("pwd", &e.to_string());
            eprintln(format!("pwd: {e}"));
            1
        }
    }
}
