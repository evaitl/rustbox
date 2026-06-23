//! Minimal SNTP client (NTP mode 3, version 4).

use crate::net::ipv4::{ipv4_to_sockaddr_in, parse_ipv4};
use crate::sys::{self, Result};
use rustix::fd::RawFd;
use rustix::io;
use std::time::{Duration, Instant};

const NTP_PORT: u16 = 123;
const NTP_PACKET_LEN: usize = 48;
/// Seconds between the NTP epoch (1900) and Unix epoch (1970).
const NTP_TO_UNIX: i64 = 2_208_988_800;

pub struct SntpResult {
    pub unix_secs: i64,
    pub unix_nsec: i64,
}

pub fn query(server: &str, timeout_secs: u32) -> Result<SntpResult> {
    let addr = parse_ipv4(server).ok_or(sys::Error::INVAL)?;
    let fd = open_udp_socket()?;
    let timeout = Duration::from_secs(u64::from(timeout_secs.max(1)));

    let mut peer = ipv4_to_sockaddr_in(addr);
    peer.sin_port = NTP_PORT.to_be();

    let mut req = [0u8; NTP_PACKET_LEN];
    req[0] = 0x23; // LI=0, VN=4, mode=3 (client)

    if unsafe {
        libc::sendto(
            fd,
            req.as_ptr() as *const libc::c_void,
            req.len(),
            0,
            &peer as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        close_fd(fd);
        return Err(err);
    }

    let mut buf = [0u8; NTP_PACKET_LEN];
    let n = recv_udp(fd, &mut buf, timeout)?;
    close_fd(fd);
    if n < NTP_PACKET_LEN {
        return Err(sys::Error::IO);
    }

    parse_response(&buf)
}

fn recv_udp(fd: RawFd, buf: &mut [u8], timeout: Duration) -> Result<usize> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(sys::Error::TIMEDOUT);
        }
        let ms = remaining.as_millis().min(i32::MAX as u128) as i32;
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut pfd as *mut libc::pollfd, 1, ms) };
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
        if (pfd.revents & libc::POLLIN) == 0 {
            continue;
        }
        let n = unsafe {
            libc::recvfrom(
                fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if n < 0 {
            let err = sys::last_errno();
            if err == sys::Error::INTR {
                continue;
            }
            return Err(err);
        }
        return Ok(n as usize);
    }
}

fn parse_response(buf: &[u8]) -> Result<SntpResult> {
    let mode = buf[0] & 0x07;
    if mode != 4 {
        return Err(sys::Error::IO);
    }
    let secs = u32::from_be_bytes(buf[40..44].try_into().map_err(|_| sys::Error::IO)?);
    let frac = u32::from_be_bytes(buf[44..48].try_into().map_err(|_| sys::Error::IO)?);
    if secs == 0 {
        return Err(sys::Error::IO);
    }
    let unix_secs = i64::from(secs) - NTP_TO_UNIX;
    let unix_nsec = (u64::from(frac) * 1_000_000_000 / (1u64 << 32)) as i64;
    Ok(SntpResult {
        unix_secs,
        unix_nsec,
    })
}

fn open_udp_socket() -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        Err(sys::last_errno())
    } else {
        Ok(fd)
    }
}

fn close_fd(fd: RawFd) {
    unsafe {
        io::close(fd);
    }
}
