//! Minimal telnet server: fork per connection, password login, PTY shell relay.
//!
//! Synchronous only (no tokio). All traffic is plaintext (see SECURITY.md).

use crate::passwd_auth::AuthPasswdTable;
use crate::passwd_lookup;
use crate::sys::{self, Error, Result};
use rustix::fd::{BorrowedFd, RawFd};
use rustix::io::{self, read, write};
use rustix::runtime::{self, Fork};
use std::path::PathBuf;

pub const DEFAULT_CONFIG: &str = "/etc/telnetd.conf";
pub const DEFAULT_PASSWD: &str = passwd_lookup::DEFAULT_PASSWD;

const MAX_LOGIN_ATTEMPTS: u32 = 3;
const MAX_LINE_BYTES: usize = 256;

const IAC: u8 = 255;
const TELNET_DONT: u8 = 254;
const TELNET_DO: u8 = 253;
const TELNET_WONT: u8 = 252;
const TELNET_WILL: u8 = 251;
const ECHO: u8 = 1;

/// IAC DONT ECHO — ask client not to echo (password prompt).
const SUPPRESS_ECHO: &[u8] = &[IAC, TELNET_DONT, ECHO];
/// IAC DO ECHO — restore echo after password.
const RESTORE_ECHO: &[u8] = &[IAC, TELNET_DO, ECHO];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub listen_addr: String,
    pub port: u16,
    pub passwd_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0".to_string(),
            port: 23,
            passwd_path: DEFAULT_PASSWD.to_string(),
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let text = sys::read_to_string(path)?;
    Ok(parse_config_text(&text))
}

pub fn parse_config_text(text: &str) -> Config {
    let mut cfg = Config::default();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        match key.to_ascii_lowercase().as_str() {
            "listen" => {
                if let Some(value) = parts.next() {
                    cfg.listen_addr = value.to_string();
                }
            }
            "port" => {
                if let Some(value) = parts.next() {
                    if let Ok(port) = value.parse::<u16>() {
                        cfg.port = port;
                    }
                }
            }
            "passwd" => {
                if let Some(value) = parts.next() {
                    cfg.passwd_path = value.to_string();
                }
            }
            _ => {}
        }
    }
    cfg
}

pub fn serve(cfg: Config, passwd: AuthPasswdTable) -> Result<()> {
    let listen_fd = listen_socket(&cfg)?;
    loop {
        let _ = sys::reap_zombies();
        let client = match accept_connection(listen_fd) {
            Ok(fd) => fd,
            Err(Error::INTR) => continue,
            Err(e) => return Err(e),
        };
        match unsafe { runtime::kernel_fork() } {
            Ok(Fork::Child(_)) => {
                close_fd(listen_fd);
                let status = if handle_session(client, &passwd).is_ok() {
                    0
                } else {
                    1
                };
                close_fd(client);
                runtime::exit_group(status);
            }
            Ok(Fork::ParentOf(_)) => close_fd(client),
            Err(_) => close_fd(client),
        }
    }
}

fn handle_session(client: RawFd, passwd: &AuthPasswdTable) -> Result<()> {
    write_all(client, b"rustbox telnetd\r\n")?;
    for attempt in 0..MAX_LOGIN_ATTEMPTS {
        if attempt > 0 {
            write_all(client, b"\r\nlogin: ")?;
        } else {
            write_all(client, b"login: ")?;
        }
        let user = read_line(client)?;
        write_all(client, b"Password: ")?;
        write_all(client, SUPPRESS_ECHO)?;
        let password = read_line(client)?;
        write_all(client, RESTORE_ECHO)?;
        write_all(client, b"\r\n")?;

        if passwd.check(&user, &password) {
            write_all(client, b"\r\n")?;
            return run_pty_shell(client);
        }
        write_all(client, b"Login incorrect\r\n")?;
    }
    Ok(())
}

/// Open a PTY, fork `rash -i` on the slave side, relay telnet ↔ master.
fn run_pty_shell(client: RawFd) -> Result<()> {
    let (master, slave) = openpty()?;
    set_winsize(slave, 80, 24)?;

    match unsafe { runtime::kernel_fork() }? {
        Fork::Child(_) => {
            close_fd(master);
            if unsafe { libc::setsid() } < 0 {
                runtime::exit_group(1);
            }
            if unsafe { libc::ioctl(slave, libc::TIOCSCTTY, 0) } < 0 {
                runtime::exit_group(1);
            }
            for fd in [0, 1, 2] {
                if unsafe { libc::dup2(slave, fd) } < 0 {
                    runtime::exit_group(1);
                }
            }
            if slave > 2 {
                close_fd(slave);
            }
            set_child_env();
            let exe =
                std::fs::read_link("/proc/self/exe").unwrap_or_else(|_| PathBuf::from("rustbox"));
            let exe = exe.to_string_lossy();
            if sys::exec_argv(&[&exe, "rash", "-i"]).is_err() {
                let _ = sys::exec_argv(&["rustbox", "rash", "-i"]);
            }
            runtime::exit_group(127);
        }
        Fork::ParentOf(child) => {
            close_fd(slave);
            relay_telnet(client, master);
            close_fd(client);
            close_fd(master);
            let _ = rustix::process::waitpid(Some(child), rustix::process::WaitOptions::empty());
            Ok(())
        }
    }
}

fn set_child_env() {
    unsafe {
        libc::setenv(c"TERM".as_ptr(), c"vt100".as_ptr(), 1);
    }
}

fn relay_telnet(client: RawFd, master: RawFd) {
    let mut client_buf = [0u8; 4096];
    let mut master_buf = [0u8; 4096];
    let mut pending = Vec::new();

    loop {
        let mut fds = [
            libc::pollfd {
                fd: client,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: master,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if n <= 0 {
            break;
        }

        if (fds[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0 {
            match read(unsafe { BorrowedFd::borrow_raw(client) }, &mut client_buf) {
                Ok(0) => break,
                Ok(n) => {
                    pending.extend_from_slice(&strip_telnet(&client_buf[..n]));
                    while !pending.is_empty() {
                        match write(unsafe { BorrowedFd::borrow_raw(master) }, &pending) {
                            Ok(0) => break,
                            Ok(w) => {
                                pending.drain(..w);
                            }
                            Err(Error::INTR) => {}
                            Err(_) => return,
                        }
                    }
                }
                Err(Error::INTR) => {}
                Err(_) => break,
            }
        }

        if (fds[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0 {
            match read(unsafe { BorrowedFd::borrow_raw(master) }, &mut master_buf) {
                Ok(0) => break,
                Ok(n) => {
                    if write_all(client, &master_buf[..n]).is_err() {
                        break;
                    }
                }
                Err(Error::INTR) => {}
                Err(_) => break,
            }
        }
    }
}

fn openpty() -> Result<(RawFd, RawFd)> {
    let mut master: libc::c_int = 0;
    let mut slave: libc::c_int = 0;
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } < 0
    {
        return Err(sys::last_errno());
    }
    Ok((master, slave))
}

fn set_winsize(fd: RawFd, cols: u32, rows: u32) -> Result<()> {
    let ws = libc::winsize {
        ws_row: rows as libc::c_ushort,
        ws_col: cols as libc::c_ushort,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    if unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) } < 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn read_line(fd: RawFd) -> Result<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 64];
    loop {
        if buf.len() >= MAX_LINE_BYTES {
            return Err(Error::INVAL);
        }
        let n = read(unsafe { BorrowedFd::borrow_raw(fd) }, &mut chunk)?;
        if n == 0 {
            return Err(Error::IO);
        }
        for &b in &chunk[..n] {
            if b == b'\n' {
                return Ok(decode_line(&buf));
            }
            if b != b'\r' {
                buf.push(b);
            }
        }
    }
}

fn decode_line(raw: &[u8]) -> String {
    let stripped = strip_telnet(raw);
    String::from_utf8_lossy(&stripped).trim().to_string()
}

fn strip_telnet(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] != IAC {
            out.push(input[i]);
            i += 1;
            continue;
        }
        if i + 1 >= input.len() {
            break;
        }
        match input[i + 1] {
            IAC => {
                out.push(IAC);
                i += 2;
            }
            TELNET_WILL | TELNET_WONT | TELNET_DO | TELNET_DONT => i += 3,
            _ => i += 2,
        }
    }
    out
}

fn listen_socket(cfg: &Config) -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(sys::last_errno());
    }
    let yes: libc::c_int = 1;
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &yes as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }
    let ip = crate::net::ipv4::parse_ipv4(&cfg.listen_addr).unwrap_or(0);
    let mut addr = crate::net::ipv4::ipv4_to_sockaddr_in(ip);
    addr.sin_port = cfg.port.to_be();
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
    if unsafe { libc::listen(fd, 8) } < 0 {
        let err = sys::last_errno();
        close_fd(fd);
        return Err(err);
    }
    Ok(fd)
}

fn accept_connection(listen_fd: RawFd) -> Result<RawFd> {
    let fd = unsafe { libc::accept(listen_fd, std::ptr::null_mut(), std::ptr::null_mut()) };
    if fd < 0 {
        return Err(sys::last_errno());
    }
    Ok(fd)
}

fn write_all(fd: RawFd, mut data: &[u8]) -> Result<()> {
    while !data.is_empty() {
        match write(unsafe { BorrowedFd::borrow_raw(fd) }, data) {
            Ok(0) => return Err(Error::IO),
            Ok(n) => data = &data[n..],
            Err(Error::INTR) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn close_fd(fd: RawFd) {
    unsafe {
        io::close(fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config() {
        let cfg = parse_config_text("listen 127.0.0.1\nport 2323\npasswd /tmp/pw\n");
        assert_eq!(cfg.listen_addr, "127.0.0.1");
        assert_eq!(cfg.port, 2323);
        assert_eq!(cfg.passwd_path, "/tmp/pw");
    }

    #[test]
    fn strip_telnet_removes_iac_sequences() {
        assert_eq!(strip_telnet(b"hello"), b"hello");
        assert_eq!(strip_telnet(b"ab\xff\xfc\x01cd"), b"abcd");
        assert_eq!(strip_telnet(b"\xff\xff"), b"\xff");
    }

    #[test]
    fn decode_line_strips_telnet() {
        assert_eq!(decode_line(b"user\xff\xfb\x01"), "user");
    }
}
