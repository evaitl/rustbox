//! DNS cache daemon: UDP listener with DoH upstream.

use crate::eprintln;
use crate::net::dns_cache::DnsCache;
use crate::net::doh::Upstream;
use crate::net::ipv4;
use crate::sys::{self, Error, Result};
use rustix::fd::RawFd;
use rustix::io;
use simple_dns::{Packet, RCODE};

pub const DEFAULT_CONFIG: &str = "/etc/dnscached.conf";
const MAX_UDP: usize = 1232;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub upstream_addrs: Vec<u32>,
    pub doh_host: String,
    pub doh_path: String,
    pub listen_addr: u32,
    pub listen_port: u16,
    pub user: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            upstream_addrs: vec![
                ipv4::parse_ipv4("8.8.8.8").unwrap(),
                ipv4::parse_ipv4("8.8.4.4").unwrap(),
            ],
            doh_host: "dns.google".to_string(),
            doh_path: "/dns-query".to_string(),
            listen_addr: ipv4::parse_ipv4("0.0.0.0").unwrap(),
            listen_port: 53,
            user: "dnscache".to_string(),
        }
    }
}

impl Config {
    pub fn upstream(&self) -> Upstream {
        Upstream {
            addrs: self.upstream_addrs.clone(),
            host: self.doh_host.clone(),
            path: self.doh_path.clone(),
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let text = sys::read_to_string(path)?;
    Ok(parse_config_text(&text))
}

pub fn parse_config_text(text: &str) -> Config {
    let mut cfg = Config {
        upstream_addrs: Vec::new(),
        ..Config::default()
    };
    let mut saw_upstream = false;
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
            "upstream" => {
                if !saw_upstream {
                    cfg.upstream_addrs.clear();
                    saw_upstream = true;
                }
                for value in parts {
                    if let Some(addr) = ipv4::parse_ipv4(value) {
                        if !cfg.upstream_addrs.contains(&addr) {
                            cfg.upstream_addrs.push(addr);
                        }
                    }
                }
            }
            "host" => {
                if let Some(value) = parts.next() {
                    cfg.doh_host = value.to_string();
                }
            }
            "path" => {
                if let Some(value) = parts.next() {
                    cfg.doh_path = value.to_string();
                }
            }
            "listen" => {
                if let Some(value) = parts.next() {
                    if let Some(addr) = ipv4::parse_ipv4(value) {
                        cfg.listen_addr = addr;
                    }
                }
            }
            "port" => {
                if let Some(value) = parts.next() {
                    if let Ok(port) = value.parse::<u16>() {
                        cfg.listen_port = port;
                    }
                }
            }
            "user" => {
                if let Some(value) = parts.next() {
                    cfg.user = value.to_string();
                }
            }
            _ => {}
        }
    }
    if cfg.upstream_addrs.is_empty() {
        cfg.upstream_addrs = Config::default().upstream_addrs;
    }
    if cfg.doh_host.is_empty() {
        cfg.doh_host = Config::default().doh_host;
    }
    if cfg.doh_path.is_empty() {
        cfg.doh_path = Config::default().doh_path;
    }
    cfg
}

pub fn serve(cfg: Config) -> Result<()> {
    let fd = bind_udp(cfg.listen_addr, cfg.listen_port)?;
    drop_daemon_privileges(&cfg.user)?;
    let upstream = cfg.upstream();
    let mut cache = DnsCache::new(512);
    let mut buf = [0u8; MAX_UDP];

    loop {
        let _ = sys::reap_zombies();
        let (n, peer) = match recv_udp(fd, &mut buf) {
            Ok(v) => v,
            Err(Error::INTR) => continue,
            Err(e) => return Err(e),
        };
        let query = &buf[..n];
        let response = match resolve_query(query, &upstream, &mut cache) {
            Ok(resp) => resp,
            Err(_) => servfail_response(query),
        };
        if response.len() > MAX_UDP {
            continue;
        }
        let _ = send_udp(fd, &response, &peer);
    }
}

fn resolve_query(query: &[u8], upstream: &Upstream, cache: &mut DnsCache) -> Result<Vec<u8>> {
    if !is_supported_query(query) {
        return Err(Error::INVAL);
    }
    if let Some(hit) = cache.lookup(query) {
        return Ok(hit);
    }
    let response = crate::net::doh::doh_query(upstream, query)?;
    if Packet::parse(&response).is_err() {
        return Err(Error::IO);
    }
    cache.store(query, &response);
    let id = u16::from_be_bytes(query[0..2].try_into().map_err(|_| Error::IO)?);
    Ok(rewrite_id(&response, id))
}

fn is_supported_query(query: &[u8]) -> bool {
    let Ok(packet) = Packet::parse(query) else {
        return false;
    };
    !packet.has_flags(simple_dns::PacketFlag::RESPONSE) && packet.questions.len() == 1
}

fn servfail_response(query: &[u8]) -> Vec<u8> {
    let Ok(packet) = Packet::parse(query) else {
        return Vec::new();
    };
    let id = packet.id();
    let question = match packet.questions.first() {
        Some(q) => q.clone().into_owned(),
        None => return Vec::new(),
    };
    let mut reply = Packet::new_reply(id);
    reply.questions.push(question);
    *reply.rcode_mut() = RCODE::ServerFailure;
    reply.build_bytes_vec().unwrap_or_default()
}

fn rewrite_id(response: &[u8], id: u16) -> Vec<u8> {
    let mut out = response.to_vec();
    if out.len() >= 2 {
        out[0..2].copy_from_slice(&id.to_be_bytes());
    }
    out
}

fn drop_daemon_privileges(user: &str) -> Result<()> {
    crate::passwd_lookup::drop_to_user(user).map_err(|err| {
        eprintln(format!("dnscached: privilege drop failed: {err}"));
        Error::PERM
    })
}

fn bind_udp(addr: u32, port: u16) -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
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
    let mut sockaddr = ipv4::ipv4_to_sockaddr_in(addr);
    sockaddr.sin_port = port.to_be();
    if unsafe {
        libc::bind(
            fd,
            &sockaddr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        close_fd(fd);
        return Err(err);
    }
    Ok(fd)
}

struct UdpPeer {
    addr: libc::sockaddr_in,
    len: libc::socklen_t,
}

fn recv_udp(fd: RawFd, buf: &mut [u8]) -> Result<(usize, UdpPeer)> {
    let mut peer: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
    let n = unsafe {
        libc::recvfrom(
            fd,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
            0,
            &mut peer as *mut libc::sockaddr_in as *mut libc::sockaddr,
            &mut len,
        )
    };
    if n < 0 {
        return Err(sys::last_errno());
    }
    Ok((n as usize, UdpPeer { addr: peer, len }))
}

fn send_udp(fd: RawFd, buf: &[u8], peer: &UdpPeer) -> Result<()> {
    let n = unsafe {
        libc::sendto(
            fd,
            buf.as_ptr() as *const libc::c_void,
            buf.len(),
            0,
            &peer.addr as *const libc::sockaddr_in as *const libc::sockaddr,
            peer.len,
        )
    };
    if n < 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn close_fd(fd: RawFd) {
    unsafe {
        io::close(fd);
    }
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_input(data: &[u8]) {
    use simple_dns::Packet;

    const MAX_LEN: usize = 4096;
    let data = if data.len() > MAX_LEN {
        &data[..MAX_LEN]
    } else {
        data
    };

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_config_text(text);
    }
    let _ = Packet::parse(data);
    let response = servfail_response(data);
    let mut cache = crate::net::dns_cache::DnsCache::new(32);
    if !response.is_empty() {
        cache.store(data, &response);
        let _ = cache.lookup(data);
    }
    crate::net::doh::fuzz_input(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_upstream_is_google() {
        let cfg = Config::default();
        assert_eq!(cfg.upstream_addrs.len(), 2);
        assert_eq!(ipv4::format_ipv4(cfg.upstream_addrs[0]), "8.8.8.8");
        assert_eq!(ipv4::format_ipv4(cfg.upstream_addrs[1]), "8.8.4.4");
        assert_eq!(cfg.doh_host, "dns.google");
    }

    #[test]
    fn parses_upstream_from_config() {
        let cfg = parse_config_text(
            "# local resolver\nupstream 1.1.1.1\nupstream 1.0.0.1\nhost cloudflare-dns.com\n",
        );
        assert_eq!(cfg.upstream_addrs.len(), 2);
        assert_eq!(ipv4::format_ipv4(cfg.upstream_addrs[0]), "1.1.1.1");
        assert_eq!(ipv4::format_ipv4(cfg.upstream_addrs[1]), "1.0.0.1");
        assert_eq!(cfg.doh_host, "cloudflare-dns.com");
    }

    #[test]
    fn empty_upstream_list_falls_back_to_google() {
        let cfg = parse_config_text("listen 127.0.0.1\n");
        assert_eq!(cfg.upstream_addrs, Config::default().upstream_addrs);
    }

    #[test]
    fn default_user_is_dnscache() {
        assert_eq!(Config::default().user, "dnscache");
    }

    #[test]
    fn parses_user_from_config() {
        let cfg = parse_config_text("user dnscache\nlisten 127.0.0.1\n");
        assert_eq!(cfg.user, "dnscache");
        assert_eq!(ipv4::format_ipv4(cfg.listen_addr), "127.0.0.1");
    }
}
