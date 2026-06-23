//! Network interface configuration via ioctl.

use crate::net::ipv4::{format_ipv4, ipv4_to_sockaddr_in, parse_ipv4};
use crate::sys::{self, Error, Result};
use std::os::fd::RawFd;

const IFF_UP: i16 = 0x1;
const IFF_LOOPBACK: i16 = 0x8;
const IFF_RUNNING: i16 = 0x40;

#[derive(Clone, Debug, Default)]
pub struct IfInfo {
    pub name: String,
    pub up: bool,
    pub running: bool,
    pub loopback: bool,
    pub addr: Option<u32>,
    pub netmask: Option<u32>,
    pub mtu: u32,
    pub hwaddr: Option<[u8; 6]>,
}

fn ioctl_fd() -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return Err(sys::last_errno());
    }
    Ok(fd)
}

fn copy_if_name(ifr: &mut libc::ifreq, name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.len() >= libc::IFNAMSIZ {
        return Err(Error::INVAL);
    }
    ifr.ifr_name = [0; libc::IFNAMSIZ];
    for (i, &b) in bytes.iter().enumerate() {
        ifr.ifr_name[i] = b as libc::c_char;
    }
    Ok(())
}

pub fn list_interface_names() -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in sys::read_dir("/sys/class/net")? {
        names.push(entry.name);
    }
    names.sort();
    Ok(names)
}

pub fn interface_exists(name: &str) -> bool {
    sys::exists(&format!("/sys/class/net/{name}"))
}

pub fn read_hwaddr(name: &str) -> Result<[u8; 6]> {
    let path = format!("/sys/class/net/{name}/address");
    let text = sys::read_to_string(&path)?;
    let mut mac = [0u8; 6];
    for (i, octet) in text.trim().split(':').enumerate() {
        if i >= 6 {
            break;
        }
        mac[i] = u8::from_str_radix(octet, 16).map_err(|_| Error::INVAL)?;
    }
    Ok(mac)
}

pub fn if_index(name: &str) -> Result<u32> {
    let path = format!("/sys/class/net/{name}/ifindex");
    let text = sys::read_to_string(&path)?;
    text.trim().parse().map_err(|_| Error::INVAL)
}

pub fn read_mtu(name: &str) -> u32 {
    let path = format!("/sys/class/net/{name}/mtu");
    sys::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1500)
}

fn ioctl_ifr(fd: RawFd, request: libc::c_ulong, ifr: &mut libc::ifreq) -> Result<()> {
    if unsafe { libc::ioctl(fd, request as libc::Ioctl, ifr) } < 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn ioctl_addr(fd: RawFd, name: &str, get: bool) -> Result<Option<u32>> {
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    copy_if_name(&mut ifr, name)?;
    if !get {
        return Err(Error::INVAL);
    }
    if ioctl_ifr(fd, libc::SIOCGIFADDR, &mut ifr).is_err() {
        return Ok(None);
    }
    let sin = unsafe { &*(&raw const ifr.ifr_ifru.ifru_addr as *const libc::sockaddr_in) };
    Ok(Some(u32::from_be(sin.sin_addr.s_addr)))
}

fn ioctl_netmask(fd: RawFd, name: &str) -> Result<Option<u32>> {
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    copy_if_name(&mut ifr, name)?;
    if ioctl_ifr(fd, libc::SIOCGIFNETMASK, &mut ifr).is_err() {
        return Ok(None);
    }
    let sin = unsafe { &*(&raw const ifr.ifr_ifru.ifru_netmask as *const libc::sockaddr_in) };
    Ok(Some(u32::from_be(sin.sin_addr.s_addr)))
}

fn ioctl_flags(fd: RawFd, name: &str) -> Result<i16> {
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    copy_if_name(&mut ifr, name)?;
    ioctl_ifr(fd, libc::SIOCGIFFLAGS, &mut ifr)?;
    Ok(unsafe { ifr.ifr_ifru.ifru_flags })
}

pub fn get_if_info(name: &str) -> Result<IfInfo> {
    if !interface_exists(name) {
        return Err(Error::NODEV);
    }
    let fd = ioctl_fd()?;
    let flags = ioctl_flags(fd, name)?;
    let addr = ioctl_addr(fd, name, true)?;
    let netmask = ioctl_netmask(fd, name)?;
    let hwaddr = read_hwaddr(name).ok();
    unsafe {
        libc::close(fd);
    }
    Ok(IfInfo {
        name: name.to_string(),
        up: flags & IFF_UP != 0,
        running: flags & IFF_RUNNING != 0,
        loopback: flags & IFF_LOOPBACK != 0,
        addr,
        netmask,
        mtu: read_mtu(name),
        hwaddr,
    })
}

pub fn set_if_addr(name: &str, addr: u32, netmask: Option<u32>) -> Result<()> {
    let fd = ioctl_fd()?;
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    copy_if_name(&mut ifr, name)?;
    let sin = ipv4_to_sockaddr_in(addr);
    unsafe {
        std::ptr::copy_nonoverlapping(
            &sin as *const libc::sockaddr_in as *const libc::c_void,
            &mut ifr.ifr_ifru.ifru_addr as *mut _ as *mut libc::c_void,
            std::mem::size_of::<libc::sockaddr_in>(),
        );
    }
    ioctl_ifr(fd, libc::SIOCSIFADDR, &mut ifr)?;
    if let Some(mask) = netmask {
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        copy_if_name(&mut ifr, name)?;
        let sin = ipv4_to_sockaddr_in(mask);
        unsafe {
            std::ptr::copy_nonoverlapping(
                &sin as *const libc::sockaddr_in as *const libc::c_void,
                &mut ifr.ifr_ifru.ifru_netmask as *mut _ as *mut libc::c_void,
                std::mem::size_of::<libc::sockaddr_in>(),
            );
        }
        ioctl_ifr(fd, libc::SIOCSIFNETMASK, &mut ifr)?;
    }
    unsafe {
        libc::close(fd);
    }
    Ok(())
}

pub fn set_if_up(name: &str, up: bool) -> Result<()> {
    let fd = ioctl_fd()?;
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    copy_if_name(&mut ifr, name)?;
    ioctl_ifr(fd, libc::SIOCGIFFLAGS, &mut ifr)?;
    let flags = unsafe { &mut ifr.ifr_ifru.ifru_flags };
    if up {
        *flags |= IFF_UP;
    } else {
        *flags &= !IFF_UP;
    }
    ioctl_ifr(fd, libc::SIOCSIFFLAGS, &mut ifr)?;
    unsafe {
        libc::close(fd);
    }
    Ok(())
}

pub fn format_ifconfig_line(info: &IfInfo) -> String {
    let mut out = String::new();
    out.push_str(&info.name);
    out.push_str(": flags=");
    let mut flag_num = 0u32;
    if info.up {
        flag_num |= 1;
    }
    if info.running {
        flag_num |= 64;
    }
    out.push_str(&flag_num.to_string());
    out.push_str("  mtu ");
    out.push_str(&info.mtu.to_string());
    out.push('\n');
    if let Some(addr) = info.addr {
        out.push_str("\tinet ");
        out.push_str(&format_ipv4(addr));
        if let Some(mask) = info.netmask {
            out.push_str("  netmask ");
            out.push_str(&format_ipv4(mask));
            let bcast = addr | !mask;
            out.push_str("  broadcast ");
            out.push_str(&format_ipv4(bcast));
        }
        out.push('\n');
    }
    if let Some(hw) = info.hwaddr {
        out.push_str("\tether ");
        for (i, b) in hw.iter().enumerate() {
            if i > 0 {
                out.push(':');
            }
            out.push_str(&format!("{b:02x}"));
        }
        out.push('\n');
    }
    let mut state = Vec::new();
    if info.up {
        state.push("UP");
    }
    if info.running {
        state.push("RUNNING");
    }
    if info.loopback {
        state.push("LOOPBACK");
    }
    if !state.is_empty() {
        out.push('\t');
        out.push_str(&state.join(" "));
        out.push('\n');
    }
    out
}

pub fn configure_interface(name: &str, addr: &str, netmask: Option<&str>, up: bool) -> Result<()> {
    if let Some(ip) = parse_ipv4(addr) {
        let mask = netmask.and_then(parse_ipv4);
        set_if_addr(name, ip, mask)?;
    }
    if up {
        set_if_up(name, true)?;
    }
    Ok(())
}
