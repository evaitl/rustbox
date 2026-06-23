use crate::sys;
use crate::{eprintln, usage};

#[cfg(target_os = "linux")]
use rustix::system::RebootCommand;

pub fn run(args: &[&str]) -> i32 {
    let mut no_sync = false;

    for arg in args {
        match *arg {
            "-n" => no_sync = true,
            "-f" => {}
            s if s.starts_with('-') => {
                usage("reboot", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("reboot", &format!("unexpected argument '{s}'"));
                return 1;
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprintln("reboot: not supported on this platform");
        return 1;
    }

    #[cfg(target_os = "linux")]
    {
        if !no_sync {
            sys::sync_filesystems();
        }
        match sys::system_reboot(RebootCommand::Restart) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("reboot: {e}"));
                1
            }
        }
    }
}
