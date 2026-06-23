use crate::sys;
use crate::{eprintln, usage};
use rustix::fd::{BorrowedFd, RawFd};
use rustix::io::{self, read};
use std::path::Path;

const DEFAULT_SOCKET: &str = "/dev/log";
const DEFAULT_LOG: &str = "/var/log/messages";

pub fn run(args: &[&str]) -> i32 {
    let mut foreground = false;
    let mut socket_path = DEFAULT_SOCKET.to_string();
    let mut log_path = DEFAULT_LOG.to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" | "-F" => foreground = true,
            "-O" => {
                i += 1;
                if i >= args.len() {
                    usage("syslogd", "option requires an argument -- 'O'");
                    return 1;
                }
                log_path = args[i].to_string();
            }
            "-s" => {
                i += 1;
                if i >= args.len() {
                    usage("syslogd", "option requires an argument -- 's'");
                    return 1;
                }
                socket_path = args[i].to_string();
            }
            "-h" | "--help" => {
                usage("syslogd", "usage: syslogd [-f] [-O LOG] [-s SOCKET]");
                return 0;
            }
            s if s.starts_with('-') => {
                usage("syslogd", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("syslogd", &format!("unexpected argument -- '{s}'"));
                return 1;
            }
        }
        i += 1;
    }

    if let Some(parent) = Path::new(&log_path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = sys::mkdir_all(parent.to_str().unwrap_or("/var/log"));
        }
    }

    if !foreground {
        if let Err(e) = sys::daemonize() {
            eprintln(format!("syslogd: {e}"));
            return 1;
        }
    }

    match serve(&socket_path, &log_path) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("syslogd: {e}"));
            1
        }
    }
}

fn serve(socket_path: &str, log_path: &str) -> sys::Result<()> {
    if sys::exists(socket_path) {
        let _ = sys::remove_file(socket_path);
    }
    if let Some(parent) = Path::new(socket_path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = sys::mkdir_all(parent.to_str().unwrap_or("/dev"));
        }
    }

    let fd = socket(AF_UNIX, SOCK_DGRAM)?;
    let addr = sockaddr_un(socket_path)?;
    if unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_un as *const libc::sockaddr,
            sockaddr_un_len(&addr),
        )
    } < 0
    {
        close_fd(fd);
        return Err(sys::last_errno());
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = match read(unsafe { BorrowedFd::borrow_raw(fd) }, &mut buf) {
            Ok(0) => continue,
            Ok(n) => n,
            Err(e) if e == rustix::io::Errno::INTR => continue,
            Err(e) => {
                close_fd(fd);
                return Err(e);
            }
        };
        let line = format_log_line(&buf[..n]);
        sys::append_file(log_path, line.as_bytes())?;
    }
}

fn format_log_line(msg: &[u8]) -> String {
    let text = String::from_utf8_lossy(msg);
    let trimmed = text.trim_end_matches(['\0', '\n', '\r']);
    format!("{trimmed}\n")
}

fn socket(domain: i32, ty: i32) -> sys::Result<RawFd> {
    let fd = unsafe { libc::socket(domain, ty, 0) };
    if fd < 0 {
        Err(sys::last_errno())
    } else {
        Ok(fd)
    }
}

fn sockaddr_un(path: &str) -> sys::Result<libc::sockaddr_un> {
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    addr.sun_family = AF_UNIX as libc::sa_family_t;
    let bytes = path.as_bytes();
    if bytes.len() >= addr.sun_path.len() {
        return Err(rustix::io::Errno::NAMETOOLONG);
    }
    for (i, &b) in bytes.iter().enumerate() {
        addr.sun_path[i] = b as libc::c_char;
    }
    Ok(addr)
}

fn sockaddr_un_len(addr: &libc::sockaddr_un) -> libc::socklen_t {
    let path_len = unsafe {
        std::ffi::CStr::from_ptr(addr.sun_path.as_ptr())
            .to_bytes()
            .len()
    };
    (std::mem::size_of::<libc::sa_family_t>() + path_len + 1) as libc::socklen_t
}

fn close_fd(fd: RawFd) {
    unsafe {
        io::close(fd);
    }
}

const AF_UNIX: i32 = libc::AF_UNIX;
const SOCK_DGRAM: i32 = libc::SOCK_DGRAM;
