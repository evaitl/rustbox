//! TCP/UDP connect and listen helpers for `nc`.

use crate::net::ipv4::{ipv4_to_sockaddr_in, parse_ipv4};
use crate::sys::{self, Result};
use rustix::fd::RawFd;
use rustix::io::{self, read, write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

pub struct Config {
    pub listen: bool,
    pub udp: bool,
    pub port: u16,
    pub host: Option<String>,
    pub timeout_secs: u32,
}

pub fn run(cfg: Config) -> Result<()> {
    if cfg.listen {
        if cfg.udp {
            udp_listen(cfg.port, cfg.timeout_secs)
        } else {
            tcp_listen(cfg.port, cfg.timeout_secs)
        }
    } else if cfg.udp {
        let host = cfg.host.as_deref().ok_or(sys::Error::INVAL)?;
        udp_connect(host, cfg.port, cfg.timeout_secs)
    } else {
        let host = cfg.host.as_deref().ok_or(sys::Error::INVAL)?;
        tcp_connect(host, cfg.port, cfg.timeout_secs)
    }
}

fn tcp_connect(host: &str, port: u16, timeout_secs: u32) -> Result<()> {
    let addr = parse_ipv4(host).ok_or(sys::Error::INVAL)?;
    let fd = tcp_socket()?;
    if timeout_secs > 0 {
        set_recv_timeout(fd, timeout_secs)?;
    }

    let mut peer = ipv4_to_sockaddr_in(addr);
    peer.sin_port = port.to_be();

    connect_with_timeout(fd, &peer, timeout_secs)?;

    relay(std::io::stdin().as_raw_fd(), fd, timeout_secs);
    close_fd(fd);
    Ok(())
}

fn tcp_listen(port: u16, timeout_secs: u32) -> Result<()> {
    let listen_fd = tcp_socket()?;
    set_reuseaddr(listen_fd)?;

    let mut addr = ipv4_to_sockaddr_in(0);
    addr.sin_port = port.to_be();
    if unsafe {
        libc::bind(
            listen_fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        close_fd(listen_fd);
        return Err(err);
    }

    if unsafe { libc::listen(listen_fd, 1) } < 0 {
        let err = sys::last_errno();
        close_fd(listen_fd);
        return Err(err);
    }

    let client_fd = unsafe { libc::accept(listen_fd, std::ptr::null_mut(), std::ptr::null_mut()) };
    if client_fd < 0 {
        return Err(sys::last_errno());
    }
    close_fd(listen_fd);

    relay(std::io::stdin().as_raw_fd(), client_fd, timeout_secs);
    close_fd(client_fd);
    Ok(())
}

fn udp_connect(host: &str, port: u16, timeout_secs: u32) -> Result<()> {
    let addr = parse_ipv4(host).ok_or(sys::Error::INVAL)?;
    let fd = udp_socket()?;
    if timeout_secs > 0 {
        set_recv_timeout(fd, timeout_secs)?;
    }

    let mut peer = ipv4_to_sockaddr_in(addr);
    peer.sin_port = port.to_be();

    if unsafe {
        libc::connect(
            fd,
            &peer as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        close_fd(fd);
        return Err(err);
    }

    udp_relay(std::io::stdin().as_raw_fd(), fd, timeout_secs);
    close_fd(fd);
    Ok(())
}

fn udp_listen(port: u16, timeout_secs: u32) -> Result<()> {
    let fd = udp_socket()?;
    set_reuseaddr(fd)?;

    let mut addr = ipv4_to_sockaddr_in(0);
    addr.sin_port = port.to_be();
    if unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        close_fd(fd);
        return Err(err);
    }

    if timeout_secs > 0 {
        set_recv_timeout(fd, timeout_secs)?;
    }

    udp_bound_relay(std::io::stdin().as_raw_fd(), fd, timeout_secs);
    close_fd(fd);
    Ok(())
}

fn udp_bound_relay(stdin_fd: RawFd, sock_fd: RawFd, timeout_secs: u32) {
    use std::io::{self, Write};
    let mut stdin_buf = [0u8; 4096];
    let mut sock_buf = [0u8; 4096];
    let mut stdin_open = true;
    let mut stdout = io::stdout();
    let mut peer: Option<libc::sockaddr_in> = None;
    let mut peer_len: libc::socklen_t = 0;
    let mut pending: Vec<u8> = Vec::new();

    loop {
        let timeout_ms = if timeout_secs > 0 {
            (timeout_secs * 1000) as i32
        } else {
            -1
        };
        let mut fds = [
            libc::pollfd {
                fd: stdin_fd,
                events: if stdin_open { libc::POLLIN } else { 0 },
                revents: 0,
            },
            libc::pollfd {
                fd: sock_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms) };
        if n <= 0 {
            break;
        }

        if stdin_open && (fds[0].revents & libc::POLLIN) != 0 {
            match read(
                unsafe { rustix::fd::BorrowedFd::borrow_raw(stdin_fd) },
                &mut stdin_buf,
            ) {
                Ok(0) => stdin_open = false,
                Ok(n) => {
                    if let (Some(peer), len) = (peer.as_ref(), peer_len) {
                        if sendto_peer(sock_fd, &stdin_buf[..n], peer, len).is_err() {
                            break;
                        }
                    } else {
                        pending.extend_from_slice(&stdin_buf[..n]);
                    }
                }
                Err(e) if e == rustix::io::Errno::INTR => {}
                Err(_) => break,
            }
        }

        if (fds[1].revents & libc::POLLIN) != 0 {
            let mut from: libc::sockaddr_in = unsafe { std::mem::zeroed() };
            let mut from_len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
            let n = unsafe {
                libc::recvfrom(
                    sock_fd,
                    sock_buf.as_mut_ptr() as *mut libc::c_void,
                    sock_buf.len(),
                    0,
                    &mut from as *mut libc::sockaddr_in as *mut libc::sockaddr,
                    &mut from_len,
                )
            };
            if n < 0 {
                let err = sys::last_errno();
                if err == rustix::io::Errno::INTR {
                    continue;
                }
                break;
            }
            if n == 0 {
                break;
            }
            peer = Some(from);
            peer_len = from_len;
            if stdout.write_all(&sock_buf[..n as usize]).is_err() {
                break;
            }
            if !pending.is_empty() {
                if let Some(peer) = peer.as_ref() {
                    if sendto_peer(sock_fd, &pending, peer, peer_len).is_err() {
                        break;
                    }
                }
                pending.clear();
            }
        }
    }
}

fn sendto_peer(
    fd: RawFd,
    data: &[u8],
    peer: &libc::sockaddr_in,
    peer_len: libc::socklen_t,
) -> Result<()> {
    let n = unsafe {
        libc::sendto(
            fd,
            data.as_ptr() as *const libc::c_void,
            data.len(),
            0,
            peer as *const libc::sockaddr_in as *const libc::sockaddr,
            peer_len,
        )
    };
    if n < 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn relay(stdin_fd: RawFd, sock_fd: RawFd, timeout_secs: u32) {
    use std::io::{self, Write};
    let mut stdin_buf = [0u8; 4096];
    let mut sock_buf = [0u8; 4096];
    let mut stdin_open = true;
    let mut stdout = io::stdout();
    let idle_limit = timeout_secs > 0;
    let idle = Duration::from_secs(u64::from(timeout_secs));
    let mut last_activity = Instant::now();

    loop {
        let timeout_ms = if idle_limit {
            let remaining = idle.saturating_sub(Instant::now().duration_since(last_activity));
            if remaining.is_zero() {
                break;
            }
            remaining.as_millis().min(i32::MAX as u128) as i32
        } else {
            -1
        };
        let mut fds = [
            libc::pollfd {
                fd: stdin_fd,
                events: if stdin_open { libc::POLLIN } else { 0 },
                revents: 0,
            },
            libc::pollfd {
                fd: sock_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms) };
        if n <= 0 {
            break;
        }

        if stdin_open && (fds[0].revents & libc::POLLIN) != 0 {
            match read(
                unsafe { rustix::fd::BorrowedFd::borrow_raw(stdin_fd) },
                &mut stdin_buf,
            ) {
                Ok(0) => stdin_open = false,
                Ok(n) => {
                    last_activity = Instant::now();
                    if write_all_stream(sock_fd, &stdin_buf[..n]).is_err() {
                        break;
                    }
                }
                Err(e) if e == rustix::io::Errno::INTR => {}
                Err(_) => break,
            }
        }

        if (fds[1].revents & libc::POLLIN) != 0 {
            match read(
                unsafe { rustix::fd::BorrowedFd::borrow_raw(sock_fd) },
                &mut sock_buf,
            ) {
                Ok(0) => break,
                Ok(n) => {
                    last_activity = Instant::now();
                    if stdout.write_all(&sock_buf[..n]).is_err() {
                        break;
                    }
                }
                Err(e) if e == rustix::io::Errno::INTR => {}
                Err(_) => break,
            }
        }
    }
}

fn udp_relay(stdin_fd: RawFd, sock_fd: RawFd, timeout_secs: u32) {
    use std::io::{self, Write};
    let mut stdin_buf = [0u8; 4096];
    let mut sock_buf = [0u8; 4096];
    let mut stdin_open = true;
    let mut stdout = io::stdout();

    loop {
        let timeout_ms = if timeout_secs > 0 {
            (timeout_secs * 1000) as i32
        } else {
            -1
        };
        let mut fds = [
            libc::pollfd {
                fd: stdin_fd,
                events: if stdin_open { libc::POLLIN } else { 0 },
                revents: 0,
            },
            libc::pollfd {
                fd: sock_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms) };
        if n <= 0 {
            break;
        }

        if stdin_open && (fds[0].revents & libc::POLLIN) != 0 {
            match read(
                unsafe { rustix::fd::BorrowedFd::borrow_raw(stdin_fd) },
                &mut stdin_buf,
            ) {
                Ok(0) => stdin_open = false,
                Ok(n) => {
                    let _ = write_all_stream(sock_fd, &stdin_buf[..n]);
                }
                Err(e) if e == rustix::io::Errno::INTR => {}
                Err(_) => break,
            }
        }

        if (fds[1].revents & libc::POLLIN) != 0 {
            match read(
                unsafe { rustix::fd::BorrowedFd::borrow_raw(sock_fd) },
                &mut sock_buf,
            ) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = stdout.write_all(&sock_buf[..n]);
                }
                Err(e) if e == rustix::io::Errno::INTR => {}
                Err(_) => break,
            }
        }
    }
}

fn write_all_stream(fd: RawFd, mut data: &[u8]) -> Result<()> {
    while !data.is_empty() {
        let n = write(unsafe { rustix::fd::BorrowedFd::borrow_raw(fd) }, data)?;
        data = &data[n..];
    }
    Ok(())
}

fn tcp_socket() -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        Err(sys::last_errno())
    } else {
        Ok(fd)
    }
}

fn udp_socket() -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        Err(sys::last_errno())
    } else {
        Ok(fd)
    }
}

fn set_reuseaddr(fd: RawFd) -> Result<()> {
    let yes: libc::c_int = 1;
    if unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &yes as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    } < 0
    {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn set_recv_timeout(fd: RawFd, timeout_secs: u32) -> Result<()> {
    let tv = libc::timeval {
        tv_sec: timeout_secs as libc::time_t,
        tv_usec: 0,
    };
    if unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &tv as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::timeval>() as libc::socklen_t,
        )
    } < 0
    {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn close_fd(fd: RawFd) {
    unsafe {
        io::close(fd);
    }
}

fn connect_with_timeout(fd: RawFd, peer: &libc::sockaddr_in, timeout_secs: u32) -> Result<()> {
    let limit = Duration::from_secs(u64::from(if timeout_secs > 0 { timeout_secs } else { 10 }));
    set_nonblocking(fd, true)?;
    let ret = unsafe {
        libc::connect(
            fd,
            peer as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };
    if ret == 0 {
        set_nonblocking(fd, false)?;
        return Ok(());
    }
    let err = sys::last_errno();
    if !matches!(err, sys::Error::INPROGRESS | sys::Error::AGAIN) {
        return Err(err);
    }

    let deadline = Instant::now() + limit;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(sys::Error::TIMEDOUT);
        }
        let ms = remaining.as_millis().min(i32::MAX as u128) as i32;
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut pfd, 1, ms) };
        if n == 0 {
            return Err(sys::Error::TIMEDOUT);
        }
        if n < 0 {
            let err = sys::last_errno();
            if err == sys::Error::INTR {
                continue;
            }
            return Err(err);
        }
        if (pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
            let code = socket_error_code(fd);
            if code == 0 {
                return Err(sys::Error::IO);
            }
            return Err(sys::Error::from_raw_os_error(code));
        }
        if (pfd.revents & libc::POLLOUT) != 0 {
            let code = socket_error_code(fd);
            if code == 0 {
                set_nonblocking(fd, false)?;
                return Ok(());
            }
            return Err(sys::Error::from_raw_os_error(code));
        }
    }
}

fn socket_error_code(fd: RawFd) -> i32 {
    let mut code: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            &mut code as *mut _ as *mut libc::c_void,
            &mut len,
        )
    } < 0
    {
        return -1;
    }
    code
}

fn set_nonblocking(fd: RawFd, nonblocking: bool) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags < 0 {
        return Err(sys::last_errno());
    }
    let new_flags = if nonblocking {
        flags | libc::O_NONBLOCK
    } else {
        flags & !libc::O_NONBLOCK
    };
    if unsafe { libc::fcntl(fd, libc::F_SETFL, new_flags) } < 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}
