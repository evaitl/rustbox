//! IPv4 parsing and formatting.

pub fn parse_ipv4(s: &str) -> Option<u32> {
    let mut parts = s.split('.');
    let a: u32 = parts.next()?.parse().ok()?;
    let b: u32 = parts.next()?.parse().ok()?;
    let c: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || [a, b, c, d].iter().any(|&o| o > 255) {
        return None;
    }
    Some((a << 24) | (b << 16) | (c << 8) | d)
}

pub fn format_ipv4(addr: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        (addr >> 24) & 0xff,
        (addr >> 16) & 0xff,
        (addr >> 8) & 0xff,
        addr & 0xff
    )
}

pub fn ipv4_to_sockaddr_in(addr: u32) -> libc::sockaddr_in {
    libc::sockaddr_in {
        sin_family: libc::AF_INET as _,
        sin_port: 0,
        sin_addr: libc::in_addr {
            s_addr: addr.to_be(),
        },
        sin_zero: [0; 8],
    }
}
