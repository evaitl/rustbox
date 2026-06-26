//! IPv4 routing via netlink and /proc/net/route.

use crate::net::iface;
use crate::net::ipv4::{format_ipv4, parse_ipv4};
use crate::sys::{self, Error, Result};
use std::os::fd::RawFd;

const AF_INET: u8 = 2;
const RTM_NEWROUTE: u16 = 24;
const RTM_DELROUTE: u16 = 25;
const RTN_UNICAST: u8 = 2;
const RTPROT_STATIC: u8 = 4;
const RT_SCOPE_UNIVERSE: u8 = 0;
const RT_TABLE_MAIN: u8 = 254;
const NLM_F_REQUEST: u16 = 1;
const NLM_F_ACK: u16 = 4;
const NLM_F_CREATE: u16 = 0x400;
const NLM_F_EXCL: u16 = 0x200;
const RTA_DST: u16 = 1;
const RTA_GATEWAY: u16 = 5;
const RTA_OIF: u16 = 4;

#[repr(C)]
struct NlMsgHdr {
    nlmsg_len: u32,
    nlmsg_type: u16,
    nlmsg_flags: u16,
    nlmsg_seq: u32,
    nlmsg_pid: u32,
}

#[repr(C)]
struct RtMsg {
    rtm_family: u8,
    rtm_dst_len: u8,
    rtm_src_len: u8,
    rtm_tos: u8,
    rtm_table: u8,
    rtm_protocol: u8,
    rtm_scope: u8,
    rtm_type: u8,
    rtm_flags: u32,
}

#[repr(C)]
struct RtAttr {
    rta_len: u16,
    rta_type: u16,
}

fn nlmsg_align(len: usize) -> usize {
    (len + 3) & !3
}

fn netlink_socket() -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_RAW, libc::NETLINK_ROUTE) };
    if fd < 0 {
        return Err(sys::last_errno());
    }
    Ok(fd)
}

fn push_attr(buf: &mut Vec<u8>, rta_type: u16, data: &[u8]) {
    let len = std::mem::size_of::<RtAttr>() + data.len();
    let aligned = nlmsg_align(len);
    let attr = RtAttr {
        rta_len: len as u16,
        rta_type,
    };
    buf.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
            &attr as *const RtAttr as *const u8,
            std::mem::size_of::<RtAttr>(),
        )
    });
    buf.extend_from_slice(data);
    let pad = aligned - len;
    buf.extend(std::iter::repeat_n(0u8, pad));
}

fn wait_ack(fd: RawFd, seq: u32) -> Result<()> {
    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
        if n < 0 {
            return Err(sys::last_errno());
        }
        if n < std::mem::size_of::<NlMsgHdr>() as isize {
            continue;
        }
        let hdr = unsafe { &*(buf.as_ptr() as *const NlMsgHdr) };
        if hdr.nlmsg_seq != seq {
            continue;
        }
        if hdr.nlmsg_type == libc::NLMSG_ERROR as u16 {
            let err = unsafe { *(buf.as_ptr().add(16) as *const i32) };
            if err == 0 {
                return Ok(());
            }
            return Err(Error::from_raw_os_error(-err));
        }
    }
}

pub fn add_route(dst: u32, dst_len: u8, gateway: Option<u32>, dev: Option<&str>) -> Result<()> {
    send_route(
        RTM_NEWROUTE,
        NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
        dst,
        dst_len,
        gateway,
        dev,
    )
}

pub fn del_route(dst: u32, dst_len: u8, gateway: Option<u32>, dev: Option<&str>) -> Result<()> {
    send_route(
        RTM_DELROUTE,
        NLM_F_REQUEST | NLM_F_ACK,
        dst,
        dst_len,
        gateway,
        dev,
    )
}

fn send_route(
    msg_type: u16,
    msg_flags: u16,
    dst: u32,
    dst_len: u8,
    gateway: Option<u32>,
    dev: Option<&str>,
) -> Result<()> {
    let fd = netlink_socket()?;
    let seq = 1u32;
    let mut buf: Vec<u8> = Vec::new();

    let hdr = NlMsgHdr {
        nlmsg_len: 0,
        nlmsg_type: msg_type,
        nlmsg_flags: msg_flags,
        nlmsg_seq: seq,
        nlmsg_pid: 0,
    };
    let rt = RtMsg {
        rtm_family: AF_INET,
        rtm_dst_len: dst_len,
        rtm_src_len: 0,
        rtm_tos: 0,
        rtm_table: RT_TABLE_MAIN,
        rtm_protocol: RTPROT_STATIC,
        rtm_scope: RT_SCOPE_UNIVERSE,
        rtm_type: RTN_UNICAST,
        rtm_flags: 0,
    };

    buf.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
            &hdr as *const NlMsgHdr as *const u8,
            std::mem::size_of::<NlMsgHdr>(),
        )
    });
    buf.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
            &rt as *const RtMsg as *const u8,
            std::mem::size_of::<RtMsg>(),
        )
    });

    if dst_len > 0 {
        push_attr(&mut buf, RTA_DST, &dst.to_be_bytes());
    }
    if let Some(gw) = gateway {
        push_attr(&mut buf, RTA_GATEWAY, &gw.to_be_bytes());
    }
    if let Some(iface) = dev {
        let idx = iface::if_index(iface)?;
        push_attr(&mut buf, RTA_OIF, &(idx as u32).to_ne_bytes());
    }

    let total = buf.len() as u32;
    unsafe {
        *(buf.as_mut_ptr() as *mut NlMsgHdr) = NlMsgHdr {
            nlmsg_len: total,
            nlmsg_type: msg_type,
            nlmsg_flags: msg_flags,
            nlmsg_seq: seq,
            nlmsg_pid: 0,
        };
    }

    let sent = unsafe { libc::send(fd, buf.as_ptr() as *const libc::c_void, buf.len(), 0) };
    if sent < 0 {
        let err = sys::last_errno();
        unsafe {
            libc::close(fd);
        }
        return Err(err);
    }
    let result = wait_ack(fd, seq);
    unsafe {
        libc::close(fd);
    }
    result
}

#[derive(Clone, Debug)]
pub struct RouteEntry {
    pub iface: String,
    pub destination: u32,
    pub gateway: u32,
    pub flags: u32,
    pub mask: u32,
}

fn hex_le_ipv4(word: &str) -> Option<u32> {
    let v = u32::from_str_radix(word, 16).ok()?;
    Some(u32::from_le(v))
}

pub fn read_routes() -> Result<Vec<RouteEntry>> {
    let text = sys::read_to_string("/proc/net/route")?;
    let mut routes = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 8 {
            continue;
        }
        let destination = hex_le_ipv4(cols[1]).unwrap_or(0);
        let gateway = hex_le_ipv4(cols[2]).unwrap_or(0);
        let flags = u32::from_str_radix(cols[3], 16).unwrap_or(0);
        let mask = hex_le_ipv4(cols[7]).unwrap_or(0);
        routes.push(RouteEntry {
            iface: cols[0].to_string(),
            destination,
            gateway,
            flags,
            mask,
        });
    }
    Ok(routes)
}

pub fn format_routes(routes: &[RouteEntry]) -> String {
    let mut out = String::from("Kernel IP routing table\n");
    out.push_str(
        "Destination     Gateway         Genmask         Flags   Metric  Ref     Use Iface\n",
    );
    for r in routes {
        out.push_str(&format!(
            "{:<15} {:<15} {:<15} {:04X}    0       0       0   {}\n",
            format_ipv4(r.destination),
            if r.gateway == 0 {
                "*".to_string()
            } else {
                format_ipv4(r.gateway)
            },
            format_ipv4(r.mask),
            r.flags,
            r.iface
        ));
    }
    out
}

pub fn add_default_gateway(gw: u32, dev: &str) -> Result<()> {
    add_route(0, 0, Some(gw), Some(dev))
}

pub fn del_default_gateway(gw: u32, dev: &str) -> Result<()> {
    del_route(0, 0, Some(gw), Some(dev))
}

pub fn add_host_route(dst: u32, gw: Option<u32>, dev: Option<&str>) -> Result<()> {
    add_route(dst, 32, gw, dev)
}

pub fn del_host_route(dst: u32, gw: Option<u32>, dev: Option<&str>) -> Result<()> {
    del_route(dst, 32, gw, dev)
}

pub fn add_net_route(dst: u32, mask: u32, gw: Option<u32>, dev: Option<&str>) -> Result<()> {
    let len = prefix_len(mask);
    add_route(dst, len, gw, dev)
}

pub fn del_net_route(dst: u32, mask: u32, gw: Option<u32>, dev: Option<&str>) -> Result<()> {
    let len = prefix_len(mask);
    del_route(dst, len, gw, dev)
}

fn prefix_len(mask: u32) -> u8 {
    if mask == 0 {
        0
    } else {
        mask.count_ones() as u8
    }
}

pub fn parse_route_target(s: &str, host: bool) -> Result<(u32, u8)> {
    if host {
        parse_ipv4(s).map(|ip| (ip, 32)).ok_or(Error::INVAL)
    } else if let Some((net, prefix)) = s.split_once('/') {
        let ip = parse_ipv4(net).ok_or(Error::INVAL)?;
        let len: u8 = prefix.parse().map_err(|_| Error::INVAL)?;
        Ok((ip, len))
    } else {
        let ip = parse_ipv4(s).ok_or(Error::INVAL)?;
        Ok((ip, 24))
    }
}
