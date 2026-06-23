//! Syslog client: send messages to a `/dev/log` UNIX datagram socket.

use crate::sys::{self, Result};
use rustix::fd::RawFd;
use rustix::io;

pub const DEFAULT_SOCKET: &str = "/dev/log";

pub fn send_message(
    socket_path: &str,
    priority: u8,
    tag: Option<&str>,
    message: &str,
) -> Result<()> {
    let body = if let Some(tag) = tag {
        format!("{tag}: {message}")
    } else {
        message.to_string()
    };
    let line = format!("<{priority}>{body}");
    send_raw(socket_path, line.as_bytes())
}

pub fn send_raw(socket_path: &str, data: &[u8]) -> Result<()> {
    let fd = socket(AF_UNIX, SOCK_DGRAM)?;
    let addr = sockaddr_un(socket_path)?;
    let n = unsafe {
        libc::sendto(
            fd,
            data.as_ptr() as *const libc::c_void,
            data.len(),
            0,
            &addr as *const libc::sockaddr_un as *const libc::sockaddr,
            sockaddr_un_len(&addr),
        )
    };
    close_fd(fd);
    if n < 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn socket(domain: i32, ty: i32) -> Result<RawFd> {
    let fd = unsafe { libc::socket(domain, ty, 0) };
    if fd < 0 {
        Err(sys::last_errno())
    } else {
        Ok(fd)
    }
}

fn sockaddr_un(path: &str) -> Result<libc::sockaddr_un> {
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
