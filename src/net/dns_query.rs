//! UDP DNS queries for the `dig` applet.

use crate::net::ipv4::{ipv4_to_sockaddr_in, parse_ipv4};
use crate::sys::{self, Result};
use rustix::fd::RawFd;
use rustix::io;
use simple_dns::{Name, Packet, Question, CLASS, QCLASS, QTYPE, TYPE};

const MAX_UDP: usize = 1232;

pub fn build_query(name: &str, qtype: QTYPE) -> Result<Vec<u8>> {
    let qname = Name::new(name).map_err(|_| sys::Error::INVAL)?;
    let mut packet = Packet::new_query(rand_id());
    packet
        .questions
        .push(Question::new(qname, qtype, QCLASS::CLASS(CLASS::IN), false));
    packet.build_bytes_vec().map_err(|_| sys::Error::IO)
}

pub fn query_udp(server: &str, port: u16, query: &[u8], timeout_ms: u32) -> Result<Vec<u8>> {
    let addr = parse_ipv4(server).ok_or(sys::Error::INVAL)?;
    let fd = open_udp_socket()?;
    set_recv_timeout(fd, timeout_ms.max(100))?;

    let mut peer = ipv4_to_sockaddr_in(addr);
    peer.sin_port = port.to_be();

    if unsafe {
        libc::sendto(
            fd,
            query.as_ptr() as *const libc::c_void,
            query.len(),
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

    let mut buf = [0u8; MAX_UDP];
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
    close_fd(fd);
    if n < 0 {
        return Err(sys::last_errno());
    }
    Ok(buf[..n as usize].to_vec())
}

pub fn parse_qtype(name: &str) -> Option<QTYPE> {
    match name.to_ascii_uppercase().as_str() {
        "A" => Some(QTYPE::TYPE(TYPE::A)),
        "AAAA" => Some(QTYPE::TYPE(TYPE::AAAA)),
        "MX" => Some(QTYPE::TYPE(TYPE::MX)),
        "TXT" => Some(QTYPE::TYPE(TYPE::TXT)),
        "NS" => Some(QTYPE::TYPE(TYPE::NS)),
        "PTR" => Some(QTYPE::TYPE(TYPE::PTR)),
        "CNAME" => Some(QTYPE::TYPE(TYPE::CNAME)),
        "SOA" => Some(QTYPE::TYPE(TYPE::SOA)),
        "ANY" => Some(QTYPE::ANY),
        _ => None,
    }
}

pub fn reverse_ipv4_name(ip: &str) -> Option<String> {
    let addr = parse_ipv4(ip)?;
    Some(format!(
        "{}.{}.{}.{}.in-addr.arpa",
        addr & 0xff,
        (addr >> 8) & 0xff,
        (addr >> 16) & 0xff,
        (addr >> 24) & 0xff
    ))
}

fn rand_id() -> u16 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_nanos() as u16) | 1)
        .unwrap_or(1)
}

fn open_udp_socket() -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        Err(sys::last_errno())
    } else {
        Ok(fd)
    }
}

fn set_recv_timeout(fd: RawFd, timeout_ms: u32) -> Result<()> {
    let tv = libc::timeval {
        tv_sec: (timeout_ms / 1000) as libc::time_t,
        tv_usec: ((timeout_ms % 1000) * 1000) as libc::suseconds_t,
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
