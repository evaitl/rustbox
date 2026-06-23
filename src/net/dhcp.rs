//! Minimal DHCP client (BOOTP/DHCP).

use crate::net::iface::{self, read_hwaddr};
use crate::net::ipv4::parse_ipv4;
use crate::net::route;
use crate::sys::{self, Error, Result};
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_MAGIC: [u8; 4] = [99, 130, 83, 99];

const DHCPDISCOVER: u8 = 1;
const DHCPOFFER: u8 = 2;
const DHCPREQUEST: u8 = 3;
const DHCPACK: u8 = 5;

const OPT_PAD: u8 = 0;
const OPT_SUBNET: u8 = 1;
const OPT_ROUTER: u8 = 3;
const OPT_REQUESTED_IP: u8 = 50;
const OPT_MESSAGE_TYPE: u8 = 53;
const OPT_SERVER_ID: u8 = 54;
const OPT_END: u8 = 255;

#[derive(Clone, Debug, Default)]
pub struct DhcpLease {
    pub ip: u32,
    pub netmask: u32,
    pub gateway: Option<u32>,
    pub server_id: u32,
}

#[repr(C)]
struct DhcpPacket {
    op: u8,
    htype: u8,
    hlen: u8,
    hops: u8,
    xid: u32,
    secs: u16,
    flags: u16,
    ciaddr: u32,
    yiaddr: u32,
    siaddr: u32,
    giaddr: u32,
    chaddr: [u8; 16],
    sname: [u8; 64],
    file: [u8; 128],
    magic: [u8; 4],
}

fn dhcp_socket(iface: &str) -> Result<RawFd> {
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
        let ifname = std::ffi::CString::new(iface).map_err(|_| Error::INVAL)?;
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_BINDTODEVICE,
            ifname.as_ptr() as *const libc::c_void,
            ifname.as_bytes_with_nul().len() as libc::socklen_t,
        );
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_BROADCAST,
            &yes as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }
    let addr = libc::sockaddr_in {
        sin_family: libc::AF_INET as _,
        sin_port: DHCP_CLIENT_PORT.to_be(),
        sin_addr: libc::in_addr { s_addr: 0 },
        sin_zero: [0; 8],
    };
    if unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
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
    Ok(fd)
}

fn set_recv_timeout(fd: RawFd, ms: u32) -> Result<()> {
    let tv = libc::timeval {
        tv_sec: (ms / 1000) as libc::time_t,
        tv_usec: ((ms % 1000) * 1000) as libc::suseconds_t,
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

fn build_packet(
    msg_type: u8,
    xid: u32,
    mac: &[u8; 6],
    requested_ip: Option<u32>,
    server_id: Option<u32>,
) -> Vec<u8> {
    let mut pkt = DhcpPacket {
        op: 1,
        htype: 1,
        hlen: 6,
        hops: 0,
        xid: xid.to_be(),
        secs: 0,
        flags: 0x8000u16.to_be(),
        ciaddr: 0,
        yiaddr: 0,
        siaddr: 0,
        giaddr: 0,
        chaddr: [0; 16],
        sname: [0; 64],
        file: [0; 128],
        magic: DHCP_MAGIC,
    };
    pkt.chaddr[..6].copy_from_slice(mac);
    let mut out = unsafe {
        std::slice::from_raw_parts(
            &pkt as *const DhcpPacket as *const u8,
            std::mem::size_of::<DhcpPacket>(),
        )
    }
    .to_vec();
    out.push(OPT_MESSAGE_TYPE);
    out.push(1);
    out.push(msg_type);
    if let Some(ip) = requested_ip {
        out.push(OPT_REQUESTED_IP);
        out.push(4);
        out.extend_from_slice(&ip.to_be_bytes());
    }
    if let Some(sid) = server_id {
        out.push(OPT_SERVER_ID);
        out.push(4);
        out.extend_from_slice(&sid.to_be_bytes());
    }
    out.push(OPT_END);
    out
}

fn parse_options(opts: &[u8]) -> (u8, Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
    let mut msg_type = 0u8;
    let mut netmask = None;
    let mut router = None;
    let mut server_id = None;
    let mut yiaddr = None;
    let mut i = 0;
    while i < opts.len() {
        let code = opts[i];
        if code == OPT_PAD {
            i += 1;
            continue;
        }
        if code == OPT_END {
            break;
        }
        if i + 1 >= opts.len() {
            break;
        }
        let len = opts[i + 1] as usize;
        if i + 2 + len > opts.len() {
            break;
        }
        let val = &opts[i + 2..i + 2 + len];
        match code {
            OPT_MESSAGE_TYPE if len >= 1 => msg_type = val[0],
            OPT_SUBNET if len == 4 => {
                netmask = Some(u32::from_be_bytes([val[0], val[1], val[2], val[3]]));
            }
            OPT_ROUTER if len >= 4 => {
                router = Some(u32::from_be_bytes([val[0], val[1], val[2], val[3]]));
            }
            OPT_SERVER_ID if len == 4 => {
                server_id = Some(u32::from_be_bytes([val[0], val[1], val[2], val[3]]));
            }
            OPT_REQUESTED_IP if len == 4 => {
                yiaddr = Some(u32::from_be_bytes([val[0], val[1], val[2], val[3]]));
            }
            _ => {}
        }
        i += 2 + len;
    }
    (msg_type, netmask, router, server_id, yiaddr)
}

fn send_dhcp(fd: RawFd, data: &[u8]) -> Result<()> {
    let dest = libc::sockaddr_in {
        sin_family: libc::AF_INET as _,
        sin_port: DHCP_SERVER_PORT.to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::MAX, // 255.255.255.255
        },
        sin_zero: [0; 8],
    };
    if unsafe {
        libc::sendto(
            fd,
            data.as_ptr() as *const libc::c_void,
            data.len(),
            0,
            &dest as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn recv_dhcp(fd: RawFd, xid: u32) -> Result<Vec<u8>> {
    let mut buf = [0u8; 1024];
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
        return Err(sys::last_errno());
    }
    if (n as usize) < std::mem::size_of::<DhcpPacket>() {
        return Err(Error::IO);
    }
    let pkt = unsafe { (buf.as_ptr() as *const DhcpPacket).read_unaligned() };
    if pkt.xid != xid.to_be() {
        return Err(Error::AGAIN);
    }
    Ok(buf[..n as usize].to_vec())
}

pub fn apply_lease(iface: &str, lease: &DhcpLease) -> Result<()> {
    let mask = if lease.netmask != 0 {
        Some(lease.netmask)
    } else {
        Some(parse_ipv4("255.255.255.0").unwrap_or(0xffffff00))
    };
    iface::set_if_addr(iface, lease.ip, mask)?;
    iface::set_if_up(iface, true)?;
    if let Some(gw) = lease.gateway {
        let _ = route::add_default_gateway(gw, iface);
    }
    Ok(())
}

pub fn dhcp_acquire(iface: &str, tries: u32, timeout_ms: u32) -> Result<DhcpLease> {
    for _ in 0..30 {
        if iface::interface_exists(iface) {
            break;
        }
        sys::sleep_seconds(0.2)?;
    }
    if !iface::interface_exists(iface) {
        return Err(Error::NODEV);
    }
    iface::set_if_up(iface, true)?;
    let mac = read_hwaddr(iface)?;
    let fd = dhcp_socket(iface)?;
    set_recv_timeout(fd, timeout_ms)?;

    let xid = (std::process::id() ^ std::process::id().rotate_left(7)) as u32;
    let mut offer_ip = 0u32;
    let mut offer_mask = 0u32;
    let mut offer_router = None;
    let mut offer_server = 0u32;

    for attempt in 0..tries.max(1) {
        let discover = build_packet(DHCPDISCOVER, xid, &mac, None, None);
        send_dhcp(fd, &discover)?;

        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
        let mut got_offer = false;
        while Instant::now() < deadline {
            match recv_dhcp(fd, xid) {
                Ok(buf) => {
                    let pkt = unsafe { (buf.as_ptr() as *const DhcpPacket).read_unaligned() };
                    let opts = &buf[std::mem::size_of::<DhcpPacket>()..];
                    let (msg_type, netmask, router, server_id, _) = parse_options(opts);
                    if msg_type == DHCPOFFER {
                        offer_ip = u32::from_be(pkt.yiaddr);
                        offer_mask = netmask.unwrap_or(0xffffff00);
                        offer_router = router;
                        offer_server = server_id.unwrap_or(u32::from_be(pkt.siaddr));
                        got_offer = true;
                        break;
                    }
                }
                Err(Error::AGAIN) => continue,
                Err(e) if e == Error::TIMEDOUT => break,
                Err(e) => {
                    unsafe {
                        libc::close(fd);
                    }
                    return Err(e);
                }
            }
        }
        if !got_offer {
            continue;
        }

        set_recv_timeout(fd, timeout_ms)?;
        let request = build_packet(DHCPREQUEST, xid, &mac, Some(offer_ip), Some(offer_server));
        send_dhcp(fd, &request)?;

        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
        while Instant::now() < deadline {
            match recv_dhcp(fd, xid) {
                Ok(buf) => {
                    let pkt = unsafe { (buf.as_ptr() as *const DhcpPacket).read_unaligned() };
                    let opts = &buf[std::mem::size_of::<DhcpPacket>()..];
                    let (msg_type, netmask, router, server_id, _) = parse_options(opts);
                    if msg_type == DHCPACK {
                        let ip = {
                            let y = u32::from_be(pkt.yiaddr);
                            if y != 0 {
                                y
                            } else {
                                offer_ip
                            }
                        };
                        let lease = DhcpLease {
                            ip,
                            netmask: netmask.unwrap_or(offer_mask),
                            gateway: router.or(offer_router),
                            server_id: server_id.unwrap_or(offer_server),
                        };
                        unsafe {
                            libc::close(fd);
                        }
                        return Ok(lease);
                    }
                }
                Err(Error::AGAIN) => continue,
                Err(Error::TIMEDOUT) => break,
                Err(e) => {
                    unsafe {
                        libc::close(fd);
                    }
                    return Err(e);
                }
            }
        }

        if attempt + 1 < tries.max(1) {
            sys::sleep_seconds(0.5)?;
        }
    }

    unsafe {
        libc::close(fd);
    }
    Err(Error::TIMEDOUT)
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_parse_packet(data: &[u8]) {
    let _ = parse_options(data);
    if data.len() >= std::mem::size_of::<DhcpPacket>() {
        let pkt = unsafe { (data.as_ptr() as *const DhcpPacket).read_unaligned() };
        let opts = &data[std::mem::size_of::<DhcpPacket>()..];
        let (msg_type, netmask, router, server_id, yiaddr) = parse_options(opts);
        let _ = (
            u32::from_be(pkt.yiaddr),
            u32::from_be(pkt.siaddr),
            u32::from_be(pkt.ciaddr),
            msg_type,
            netmask,
            router,
            server_id,
            yiaddr,
        );
    }
}
