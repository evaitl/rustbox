use crate::mdev::{self, Action, DEFAULT_CONF};
use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut scan = false;
    let mut daemon = false;
    let mut foreground = false;
    let mut config_path = DEFAULT_CONF.to_string();

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == "-c" {
            i += 1;
            if i >= args.len() {
                usage("mdev", "option requires an argument -- 'c'");
                return 1;
            }
            config_path = args[i].to_string();
        } else if arg == "-h" || arg == "--help" {
            usage(
                "mdev",
                "usage: mdev [-s] [-d] [-f] [-c CONFIG]\n\
                 \t-s\tscan /sys and apply /etc/mdev.conf\n\
                 \t-d\tlisten for kernel uevents (USB hotplug)\n\
                 \t-f\tstay in foreground (with -d)\n\
                 \t-c CONFIG\tconfig file (default /etc/mdev.conf)",
            );
            return 0;
        } else if let Some(flags) = arg.strip_prefix('-') {
            if flags.is_empty() || flags.starts_with('-') {
                usage("mdev", &format!("invalid option -- '{arg}'"));
                return 1;
            }
            for ch in flags.chars() {
                match ch {
                    's' => scan = true,
                    'd' => daemon = true,
                    'f' => foreground = true,
                    'c' => {
                        usage("mdev", "option requires an argument -- 'c'");
                        return 1;
                    }
                    _ => {
                        usage("mdev", &format!("invalid option -- '{arg}'"));
                        return 1;
                    }
                }
            }
        } else {
            usage("mdev", &format!("unexpected argument -- '{arg}'"));
            return 1;
        }
        i += 1;
    }

    let rules = mdev::load_rules(&config_path);

    if scan {
        if let Err(e) = mdev::scan(&rules) {
            eprintln(format!("mdev: scan: {e}"));
            return 1;
        }
        if !daemon {
            return 0;
        }
    }

    if daemon {
        if !foreground {
            if let Err(e) = sys::daemonize() {
                eprintln(format!("mdev: {e}"));
                return 1;
            }
        }
        match mdev::serve_daemon(&rules) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("mdev: {e}"));
                1
            }
        }
    } else if std::env::var("ACTION").is_ok() {
        let ev = mdev::parse_hotplug_env();
        let action = ev.action.unwrap_or(Action::Add);
        match mdev::handle_event(&ev, &rules, action) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("mdev: {e}"));
                1
            }
        }
    } else if !scan {
        usage(
            "mdev",
            "usage: mdev [-s] [-d] [-f] [-c CONFIG]  (or run from kernel hotplug with ACTION set)",
        );
        1
    } else {
        0
    }
}
