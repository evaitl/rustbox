//! ICMP echo (ping) over Linux ping sockets.

use crate::net::ipv4::{ipv4_to_sockaddr_in, parse_ipv4};
use crate::sys::{self, Error, Result};
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

const ICMP_ECHO: u8 = 8;
const ICMP_ECHOREPLY: u8 = 0;

#[repr(C)]
struct IcmpHdr {
    icmp_type: u8,
    icmp_code: u8,
    icmp_cksum: u16,
    icmp_id: u16,
    icmp_seq: u16,
}

fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        let word = u16::from_be_bytes([data[i], data[i + 1]]);
        sum = sum.wrapping_add(u32::from(word));
        i += 2;
    }
    if i < data.len() {
        sum = sum.wrapping_add(u32::from(data[i]) << 8);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !sum as u16
}

fn set_recv_timeout(fd: RawFd, secs: u32) -> Result<()> {
    let tv = libc::timeval {
        tv_sec: secs as libc::time_t,
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

fn enable_ping_sockets() {
    if let Ok(fd) = sys::open_create("/proc/sys/net/ipv4/ping_group_range") {
        let _ = rustix::io::write(fd, b"0\t2147483647\n");
    }
}

fn get_icmp_id(fd: RawFd) -> Result<u16> {
    let mut addr: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
    if unsafe { libc::getsockname(fd, &mut addr as *mut _ as *mut libc::sockaddr, &mut len) } < 0 {
        return Err(sys::last_errno());
    }
    // Linux ping sockets use sin_port as the ICMP identifier (network byte order).
    Ok(addr.sin_port)
}

fn open_icmp_socket() -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_ICMP) };
    if fd >= 0 {
        return Ok(fd);
    }
    let err = sys::last_errno();
    if matches!(err, Error::ACCESS | Error::PERM) {
        enable_ping_sockets();
        let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_ICMP) };
        if fd >= 0 {
            return Ok(fd);
        }
    }
    Err(sys::last_errno())
}

pub fn ping_host(host: &str, count: u32, timeout_secs: u32, quiet: bool) -> Result<()> {
    let addr = parse_ipv4(host).ok_or(Error::INVAL)?;
    let fd = open_icmp_socket()?;
    set_recv_timeout(fd, timeout_secs.max(1))?;

    let dest = ipv4_to_sockaddr_in(addr);
    let mut icmp_id = 0u16;
    let mut received = 0u32;

    for seq in 0..count {
        let mut hdr = IcmpHdr {
            icmp_type: ICMP_ECHO,
            icmp_code: 0,
            icmp_cksum: 0,
            icmp_id: 0,
            icmp_seq: (seq as u16).to_be(),
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &hdr as *const IcmpHdr as *const u8,
                std::mem::size_of::<IcmpHdr>(),
            )
        };
        hdr.icmp_cksum = checksum(bytes).to_be();
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &hdr as *const IcmpHdr as *const u8,
                std::mem::size_of::<IcmpHdr>(),
            )
        };

        let start = Instant::now();
        if unsafe {
            libc::sendto(
                fd,
                bytes.as_ptr() as *const libc::c_void,
                bytes.len(),
                0,
                &dest as *const libc::sockaddr_in as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            )
        } < 0
        {
            let err = sys::last_errno();
            unsafe {
                libc::close(fd);
            }
            return Err(err);
        }

        if icmp_id == 0 {
            icmp_id = get_icmp_id(fd)?;
        }

        let mut buf = [0u8; 128];
        let mut got_reply = false;
        loop {
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
                if err == Error::AGAIN || err == Error::TIMEDOUT {
                    if !quiet {
                        println!("ping: {host}: Request timeout");
                    }
                    break;
                }
                unsafe {
                    libc::close(fd);
                }
                return Err(err);
            }
            if n >= std::mem::size_of::<IcmpHdr>() as isize {
                let reply = unsafe { &*(buf.as_ptr() as *const IcmpHdr) };
                if reply.icmp_type == ICMP_ECHOREPLY
                    && reply.icmp_id == icmp_id
                    && reply.icmp_seq == (seq as u16).to_be()
                {
                    if !quiet {
                        let ms = start.elapsed().as_secs_f64() * 1000.0;
                        println!(
                            "{} bytes from {}: icmp_seq={} ttl=64 time={ms:.1} ms",
                            n, host, seq
                        );
                    }
                    received += 1;
                    got_reply = true;
                    break;
                }
            }
            if start.elapsed() >= Duration::from_secs(timeout_secs.max(1) as u64) {
                if !quiet {
                    println!("ping: {host}: Request timeout");
                }
                break;
            }
        }
        if !got_reply && quiet {
            unsafe {
                libc::close(fd);
            }
            return Err(Error::TIMEDOUT);
        }
    }

    unsafe {
        libc::close(fd);
    }

    if !quiet {
        println!(
            "--- {host} ping statistics ---\n{count} packets transmitted, {received} received, {:.0}% packet loss",
            if count == 0 {
                0.0
            } else {
                ((count - received) as f64 / count as f64) * 100.0
            }
        );
    }

    if received == 0 {
        return Err(Error::TIMEDOUT);
    }
    Ok(())
}
