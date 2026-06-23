use crate::sys;

pub fn run(_args: &[&str]) -> i32 {
    sys::sync_all();
    0
}
