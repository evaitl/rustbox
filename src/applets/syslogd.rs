use crate::sys;
use crate::{eprintln, usage};
use rustix::fd::{AsRawFd, BorrowedFd, OwnedFd, RawFd};
use rustix::fs::{open, Mode, OFlags};
use rustix::io::{self, read, Errno};
use std::path::Path;

const DEFAULT_SOCKET: &str = "/dev/log";
const DEFAULT_LOG: &str = "/var/log/messages";
const DEFAULT_KMSG: &str = "/dev/kmsg";

pub fn run(args: &[&str]) -> i32 {
    let mut foreground = false;
    let mut klog = false;
    let mut socket_path = DEFAULT_SOCKET.to_string();
    let mut log_path = DEFAULT_LOG.to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" | "-F" => foreground = true,
            "-k" => klog = true,
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
                usage("syslogd", "usage: syslogd [-f] [-k] [-O LOG] [-s SOCKET]");
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

    match serve(&socket_path, &log_path, klog) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("syslogd: {e}"));
            1
        }
    }
}

fn serve(socket_path: &str, log_path: &str, klog: bool) -> sys::Result<()> {
    if sys::exists(socket_path) {
        let _ = sys::remove_file(socket_path);
    }
    if let Some(parent) = Path::new(socket_path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = sys::mkdir_all(parent.to_str().unwrap_or("/dev"));
        }
    }

    let sock_fd = socket(AF_UNIX, SOCK_DGRAM)?;
    let addr = sockaddr_un(socket_path)?;
    if unsafe {
        libc::bind(
            sock_fd,
            &addr as *const libc::sockaddr_un as *const libc::sockaddr,
            sockaddr_un_len(&addr),
        )
    } < 0
    {
        close_fd(sock_fd);
        return Err(sys::last_errno());
    }

    let kmsg_fd = if klog { Some(open_kmsg()?) } else { None };

    let mut buf = [0u8; 4096];
    loop {
        let mut fds = [
            libc::pollfd {
                fd: sock_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: kmsg_fd.as_ref().map_or(-1, |fd| fd.as_raw_fd()),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let nfds = if klog { 2 } else { 1 };
        loop {
            let ret = unsafe { libc::poll(fds.as_mut_ptr(), nfds, -1) };
            if ret < 0 {
                let err = sys::last_errno();
                if err == Errno::INTR {
                    continue;
                }
                close_fd(sock_fd);
                return Err(err);
            }
            break;
        }

        if fds[0].revents & libc::POLLIN != 0 {
            match read(unsafe { BorrowedFd::borrow_raw(sock_fd) }, &mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    let line = format_log_line(&buf[..n]);
                    sys::append_file(log_path, line.as_bytes())?;
                }
                Err(e) if e == Errno::INTR => {}
                Err(e) => {
                    close_fd(sock_fd);
                    return Err(e);
                }
            }
        }

        if klog && fds[1].revents & libc::POLLIN != 0 {
            if let Some(kmsg) = &kmsg_fd {
                drain_kmsg(kmsg, log_path, &mut buf)?;
            }
        }
    }
}

fn open_kmsg() -> sys::Result<OwnedFd> {
    open(
        DEFAULT_KMSG,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

fn drain_kmsg(kmsg: &OwnedFd, log_path: &str, buf: &mut [u8]) -> sys::Result<()> {
    loop {
        match read(kmsg, &mut buf[..]) {
            Ok(0) => break,
            Ok(n) => {
                let line = format_kmsg_line(&buf[..n]);
                sys::append_file(log_path, line.as_bytes())?;
            }
            Err(Errno::AGAIN) => break,
            Err(Errno::INTR) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn format_log_line(msg: &[u8]) -> String {
    let text = String::from_utf8_lossy(msg);
    let trimmed = text.trim_end_matches(['\0', '\n', '\r']);
    format!("{trimmed}\n")
}

fn format_kmsg_line(msg: &[u8]) -> String {
    let text = String::from_utf8_lossy(msg);
    let trimmed = text.trim_end_matches(['\0', '\n', '\r']);
    let body = trimmed
        .split_once(';')
        .map(|(_, message)| message.trim())
        .filter(|message| !message.is_empty())
        .unwrap_or(trimmed);
    format!("kernel: {body}\n")
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
        return Err(Errno::NAMETOOLONG);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmsg_line_strips_header() {
        let line = format_kmsg_line(b"6,123,4567890123,-;usb 1-1: new device\n");
        assert_eq!(line, "kernel: usb 1-1: new device\n");
    }

    #[test]
    fn kmsg_line_without_header_is_passthrough() {
        let line = format_kmsg_line(b"plain kernel text\n");
        assert_eq!(line, "kernel: plain kernel text\n");
    }
}
