pub mod applets;
pub mod compress;
#[cfg(all(target_os = "linux", any(applet_mdev, test)))]
pub mod mdev;
#[cfg(target_os = "linux")]
pub mod net;
#[cfg(any(feature = "applet-passwd", feature = "applet-sshd"))]
pub mod passwd_auth;
pub mod passwd_lookup;
pub mod sys;

#[cfg(feature = "fuzzing")]
pub mod fuzz {
    #[cfg(applet_sh)]
    pub use crate::applets::sh::fuzz::{rash_arith, rash_parse, rash_run};

    #[cfg(all(applet_udhcpc, target_os = "linux"))]
    pub fn udhcpc(data: &[u8]) {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const MAX_LEN: usize = 4096;
        let data = if data.len() > MAX_LEN {
            &data[..MAX_LEN]
        } else {
            data
        };

        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(input) = std::str::from_utf8(data) {
                crate::applets::udhcpc::fuzz_parse_args(input);
            }
            crate::net::dhcp::fuzz_parse_packet(data);
        }));
    }

    #[cfg(all(applet_thttpd, target_os = "linux"))]
    pub fn thttpd(data: &[u8]) {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const MAX_LEN: usize = 16 * 1024;
        let data = if data.len() > MAX_LEN {
            &data[..MAX_LEN]
        } else {
            data
        };

        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(input) = std::str::from_utf8(data) {
                crate::applets::thttpd::fuzz_parse_args(input);
            }
            crate::net::http::fuzz_input(data);
        }));
    }

    #[cfg(all(applet_wget, target_os = "linux"))]
    pub fn wget(data: &[u8]) {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const MAX_LEN: usize = 4096;
        let data = if data.len() > MAX_LEN {
            &data[..MAX_LEN]
        } else {
            data
        };

        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(input) = std::str::from_utf8(data) {
                crate::applets::wget::fuzz_parse_args(input);
            }
            crate::net::http_client::fuzz_input(data);
        }));
    }
    #[cfg(all(applet_dnscached, feature = "applet-dnscached", target_os = "linux"))]
    pub fn dnscached(data: &[u8]) {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const MAX_LEN: usize = 4096;
        let data = if data.len() > MAX_LEN {
            &data[..MAX_LEN]
        } else {
            data
        };

        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(input) = std::str::from_utf8(data) {
                crate::applets::dnscached::fuzz_parse_args(input);
            }
            crate::net::dnscached::fuzz_input(data);
        }));
    }

    #[cfg(all(applet_sshd, feature = "applet-sshd", target_os = "linux"))]
    pub fn sshd(data: &[u8]) {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const MAX_LEN: usize = 16 * 1024;
        let data = if data.len() > MAX_LEN {
            &data[..MAX_LEN]
        } else {
            data
        };

        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(input) = std::str::from_utf8(data) {
                crate::applets::sshd::fuzz_parse_args(input);
            }
            crate::net::sshd::fuzz_input(data);
        }));
    }

    #[cfg(applet_gzip)]
    pub fn gzip(data: &[u8]) {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const MAX_LEN: usize = 64 * 1024;
        let data = if data.len() > MAX_LEN {
            &data[..MAX_LEN]
        } else {
            data
        };

        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(input) = std::str::from_utf8(data) {
                crate::applets::gzip::fuzz_parse_args(input);
            }
            crate::applets::gzip::fuzz_input(data);
        }));
    }

    #[cfg(applet_tar)]
    pub fn tar(data: &[u8]) {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const MAX_LEN: usize = 128 * 1024;
        let data = if data.len() > MAX_LEN {
            &data[..MAX_LEN]
        } else {
            data
        };

        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(input) = std::str::from_utf8(data) {
                crate::applets::tar::fuzz_parse_args(input);
            }
            crate::applets::tar::fuzz_input(data);
        }));
    }
}

use std::io::{self, Write};
use std::path::Path;

pub type AppletFn = fn(&[&str]) -> i32;

pub struct Applet {
    pub name: &'static str,
    pub run: AppletFn,
}

include!(concat!(env!("OUT_DIR"), "/applets_table.rs"));

pub fn find_applet(name: &str) -> Option<AppletFn> {
    APPLETS.iter().find(|a| a.name == name).map(|a| a.run)
}

pub fn applet_names() -> impl Iterator<Item = &'static str> {
    APPLETS.iter().map(|a| a.name)
}

/// Basename of argv[0], stripping a `.exe` suffix on Windows.
pub fn invocation_name(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.strip_suffix(".exe").unwrap_or(s))
        .unwrap_or("rustbox")
        .to_string()
}

pub fn eprintln(args: impl AsRef<str>) {
    let _ = writeln!(io::stderr(), "{}", args.as_ref());
}

pub fn usage(applet: &str, msg: &str) {
    eprintln(format!("{applet}: {msg}"));
}
