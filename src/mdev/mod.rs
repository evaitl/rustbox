//! Minimal mdev: sysfs scan, hotplug, and kobject uevent daemon.

use crate::sys::{self, Error, Result};
use rustix::fs::{FileType, Mode};
use rustix::process::{Gid, Uid};
use std::path::Path;

pub const DEFAULT_CONF: &str = "/etc/mdev.conf";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Add,
    Remove,
    Change,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Uevent {
    pub action: Option<Action>,
    pub devpath: String,
    pub subsystem: Option<String>,
    pub major: Option<u32>,
    pub minor: Option<u32>,
    pub devname: Option<String>,
    pub devtype: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub pattern: String,
    pub uid: Option<Uid>,
    pub gid: Option<Gid>,
    pub mode: Mode,
    pub alias: Option<String>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            pattern: ".*".to_string(),
            uid: Some(Uid::ROOT),
            gid: Some(Gid::ROOT),
            mode: Mode::from_raw_mode(0o660),
            alias: None,
        }
    }
}

pub fn load_rules(path: &str) -> Vec<Rule> {
    match sys::read_to_string(path) {
        Ok(text) => parse_conf_text(&text),
        Err(_) => Vec::new(),
    }
}

pub fn parse_conf_text(text: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rule) = parse_conf_line(line) {
            rules.push(rule);
        }
    }
    rules
}

fn parse_conf_line(line: &str) -> Option<Rule> {
    let mut parts = line.split_whitespace();
    let pattern = parts.next()?.to_string();
    let owner = parts.next()?;
    let mode_str = parts.next()?;
    let (uid, gid) = parse_owner(owner)?;
    let mode = parse_mode(mode_str)?;
    let alias = parts
        .next()
        .map(|s| s.strip_prefix('=').unwrap_or(s).to_string());
    Some(Rule {
        pattern,
        uid,
        gid,
        mode,
        alias,
    })
}

fn parse_owner(s: &str) -> Option<(Option<Uid>, Option<Gid>)> {
    let (u, g) = s.split_once(':')?;
    let uid = if u == "*" {
        None
    } else {
        Some(Uid::from_raw(u.parse().ok()?))
    };
    let gid = if g == "*" {
        None
    } else {
        Some(Gid::from_raw(g.parse().ok()?))
    };
    Some((uid, gid))
}

fn parse_mode(s: &str) -> Option<Mode> {
    u32::from_str_radix(s, 8).ok().map(Mode::from_raw_mode)
}

pub fn parse_uevent_text(text: &str) -> Uevent {
    let mut ev = Uevent::default();
    for line in text.lines() {
        let line = line.trim_end_matches('\0');
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "ACTION" => ev.action = parse_action(value),
                "DEVPATH" => ev.devpath = value.to_string(),
                "SUBSYSTEM" => ev.subsystem = Some(value.to_string()),
                "MAJOR" => ev.major = value.parse().ok(),
                "MINOR" => ev.minor = value.parse().ok(),
                "DEVNAME" => ev.devname = Some(value.to_string()),
                "DEVTYPE" => ev.devtype = Some(value.to_string()),
                _ => {}
            }
        }
    }
    ev
}

pub fn parse_hotplug_env() -> Uevent {
    let mut ev = Uevent::default();
    for (key, value) in std::env::vars() {
        match key.as_str() {
            "ACTION" => ev.action = parse_action(&value),
            "DEVPATH" => ev.devpath = value,
            "SUBSYSTEM" => ev.subsystem = Some(value),
            "MAJOR" => ev.major = value.parse().ok(),
            "MINOR" => ev.minor = value.parse().ok(),
            "DEVNAME" => ev.devname = Some(value),
            "DEVTYPE" => ev.devtype = Some(value),
            _ => {}
        }
    }
    ev
}

pub fn parse_netlink_message(buf: &[u8]) -> Option<Uevent> {
    let text = std::str::from_utf8(buf).ok()?;
    let at = text.find('@')?;
    let header = &text[..at];
    let action = parse_action(header)?;
    let devpath = text[at + 1..]
        .split('\0')
        .next()
        .unwrap_or("")
        .trim_start_matches('/')
        .trim_start_matches("devices/")
        .to_string();
    let devpath = if devpath.starts_with('/') {
        devpath
    } else {
        format!("/devices/{devpath}")
    };

    let mut ev = Uevent {
        action: Some(action),
        devpath,
        ..Default::default()
    };

    for line in text.split('\0') {
        let line = line.trim();
        if line.is_empty() || line.contains('@') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "SUBSYSTEM" => ev.subsystem = Some(value.to_string()),
                "MAJOR" => ev.major = value.parse().ok(),
                "MINOR" => ev.minor = value.parse().ok(),
                "DEVNAME" => ev.devname = Some(value.to_string()),
                "DEVTYPE" => ev.devtype = Some(value.to_string()),
                _ => {}
            }
        }
    }
    Some(ev)
}

fn parse_action(s: &str) -> Option<Action> {
    match s {
        "add" => Some(Action::Add),
        "remove" => Some(Action::Remove),
        "change" => Some(Action::Change),
        _ => None,
    }
}

pub fn glob_match(pattern: &str, name: &str) -> bool {
    glob_match_impl(pattern.as_bytes(), name.as_bytes())
}

fn glob_match_impl(pattern: &[u8], name: &[u8]) -> bool {
    match (pattern.first(), name.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            glob_match_impl(&pattern[1..], name)
                || (!name.is_empty() && glob_match_impl(pattern, &name[1..]))
        }
        (Some(b'?'), Some(_)) => glob_match_impl(&pattern[1..], &name[1..]),
        (Some(b'['), Some(ch)) => match parse_bracket_class(&pattern[1..], *ch) {
            Some((consumed, ok)) if ok => glob_match_impl(&pattern[1 + consumed..], &name[1..]),
            _ => false,
        },
        (Some(p), Some(n)) if p == n => glob_match_impl(&pattern[1..], &name[1..]),
        _ => false,
    }
}

fn parse_bracket_class(pattern: &[u8], ch: u8) -> Option<(usize, bool)> {
    let end = pattern.iter().position(|&b| b == b']')?;
    let class = &pattern[..end];
    let matched = if class.first() == Some(&b'!') {
        !char_in_class(&class[1..], ch)
    } else {
        char_in_class(class, ch)
    };
    Some((end + 1, matched))
}

fn char_in_class(class: &[u8], ch: u8) -> bool {
    let mut i = 0;
    while i < class.len() {
        if i + 2 < class.len() && class[i + 1] == b'-' {
            let start = class[i];
            let end = class[i + 2];
            if ch >= start && ch <= end {
                return true;
            }
            i += 3;
        } else if class[i] == ch {
            return true;
        } else {
            i += 1;
        }
    }
    false
}

pub fn match_rule(rules: &[Rule], name: &str) -> Rule {
    let mut selected = Rule::default();
    let mut matched = false;
    for rule in rules {
        if glob_match(&rule.pattern, name) {
            selected = rule.clone();
            matched = true;
        }
    }
    if matched {
        selected
    } else {
        Rule::default()
    }
}

pub fn scan(rules: &[Rule]) -> Result<()> {
    scan_dir("/sys/devices", rules)
}

fn scan_dir(path: &str, rules: &[Rule]) -> Result<()> {
    let uevent_path = format!("{path}/uevent");
    if sys::exists(&uevent_path) {
        let text = sys::read_to_string(&uevent_path)?;
        let mut ev = parse_uevent_text(&text);
        if ev.devpath.is_empty() {
            ev.devpath = sysfs_devpath(path);
        }
        if ev.major.is_some() {
            let _ = handle_event(&ev, rules, Action::Add);
        }
    }

    let entries = match sys::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries {
        if entry.file_type.is_dir() {
            scan_dir(&format!("{path}/{}", entry.name), rules)?;
        }
    }
    Ok(())
}

pub fn handle_event(ev: &Uevent, rules: &[Rule], action: Action) -> Result<()> {
    let action = ev.action.unwrap_or(action);
    let Some(major) = ev.major else {
        return Ok(());
    };
    let Some(minor) = ev.minor else {
        return Ok(());
    };

    let devname = device_name(ev);
    let rule = match_rule(rules, &devname);

    let node = format!("/dev/{devname}");
    match action {
        Action::Remove => {
            let _ = sys::remove_file(&node);
            if let Some(alias) = &rule.alias {
                let _ = sys::remove_file(&format!("/dev/{alias}"));
            }
            return Ok(());
        }
        Action::Add | Action::Change => {}
    }

    let file_type = device_file_type(ev);
    if !sys::exists(&node) {
        sys::mknod(&node, file_type, rule.mode, major, minor)?;
    }
    if let Some(uid) = rule.uid {
        sys::chown_path(&node, Some(uid), rule.gid)?;
    } else if let Some(gid) = rule.gid {
        sys::chown_path(&node, None, Some(gid))?;
    }
    let _ = sys::chmod_path(&node, rule.mode);

    if let Some(alias) = &rule.alias {
        let link = format!("/dev/{alias}");
        if sys::exists(&link) {
            let _ = sys::remove_file(&link);
        }
        let _ = sys::sym_link(&devname, &link);
    }
    Ok(())
}

fn device_name(ev: &Uevent) -> String {
    if let Some(name) = &ev.devname {
        return name.clone();
    }
    ev.devpath
        .rsplit('/')
        .next()
        .unwrap_or("device")
        .to_string()
}

fn device_file_type(ev: &Uevent) -> FileType {
    if ev.subsystem.as_deref() == Some("block") || ev.devpath.contains("/block/") {
        FileType::BlockDevice
    } else {
        FileType::CharacterDevice
    }
}

fn sysfs_devpath(sys_path: &str) -> String {
    Path::new(sys_path)
        .strip_prefix("/sys")
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| sys_path.to_string())
}

pub fn serve_daemon(rules: &[Rule]) -> Result<()> {
    let fd = open_uevent_socket()?;
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
        if n < 0 {
            let err = sys::last_errno();
            if err == Error::INTR {
                continue;
            }
            return Err(err);
        }
        if n == 0 {
            continue;
        }
        let Some(mut ev) = parse_netlink_message(&buf[..n as usize]) else {
            continue;
        };
        if ev.devpath.is_empty() {
            continue;
        }
        if ev.major.is_none() {
            enrich_from_sysfs(&mut ev);
        }
        let action = ev.action.unwrap_or(Action::Add);
        let _ = handle_event(&ev, rules, action);
    }
}

fn enrich_from_sysfs(ev: &mut Uevent) {
    let path = format!("/sys{}", ev.devpath);
    let uevent_path = format!("{path}/uevent");
    if let Ok(text) = sys::read_to_string(&uevent_path) {
        let parsed = parse_uevent_text(&text);
        if ev.major.is_none() {
            ev.major = parsed.major;
        }
        if ev.minor.is_none() {
            ev.minor = parsed.minor;
        }
        if ev.devname.is_none() {
            ev.devname = parsed.devname;
        }
        if ev.subsystem.is_none() {
            ev.subsystem = parsed.subsystem;
        }
        if ev.devtype.is_none() {
            ev.devtype = parsed.devtype;
        }
    }
}

const NETLINK_KOBJECT_UEVENT: i32 = 15;

#[repr(C)]
struct SockaddrNl {
    nl_family: u16,
    nl_pad: u16,
    nl_pid: u32,
    nl_groups: u32,
}

fn open_uevent_socket() -> Result<libc::c_int> {
    let fd = unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_DGRAM, NETLINK_KOBJECT_UEVENT) };
    if fd < 0 {
        return Err(sys::last_errno());
    }

    let bufsize: libc::c_int = 128 * 1024;
    let _ = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &bufsize as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };

    let addr = SockaddrNl {
        nl_family: libc::AF_NETLINK as u16,
        nl_pad: 0,
        nl_pid: 0,
        nl_groups: 1,
    };
    if unsafe {
        libc::bind(
            fd,
            &addr as *const SockaddrNl as *const libc::sockaddr,
            std::mem::size_of::<SockaddrNl>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        unsafe { libc::close(fd) };
        return Err(err);
    }
    Ok(fd)
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_input(data: &[u8]) {
    const MAX_LEN: usize = 16 * 1024;
    let data = if data.len() > MAX_LEN {
        &data[..MAX_LEN]
    } else {
        data
    };
    if let Ok(text) = std::str::from_utf8(data) {
        let rules = parse_conf_text(text);
        let _ = parse_uevent_text(text);
        if !rules.is_empty() {
            let _ = glob_match(&rules[0].pattern, "sda1");
        }
    }
    let _ = parse_netlink_message(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_conf_line() {
        let rules = parse_conf_text("sd[a-z] 0:0 660\n# comment\nttyUSB* 0:0 666 =usbserial\n");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].pattern, "sd[a-z]");
        assert_eq!(rules[0].mode, Mode::from_raw_mode(0o660));
        assert_eq!(rules[1].alias.as_deref(), Some("usbserial"));
    }

    #[test]
    fn glob_matches_usb_storage() {
        assert!(glob_match("sd[a-z]", "sda"));
        assert!(glob_match("sd[a-z][0-9]", "sda1"));
        assert!(glob_match("ttyUSB*", "ttyUSB0"));
        assert!(!glob_match("sd[a-z]", "hda"));
    }

    #[test]
    fn parses_uevent() {
        let ev = parse_uevent_text(
            "MAJOR=8\nMINOR=1\nDEVNAME=sda1\nSUBSYSTEM=block\nDEVTYPE=partition\n",
        );
        assert_eq!(ev.major, Some(8));
        assert_eq!(ev.minor, Some(1));
        assert_eq!(ev.devname.as_deref(), Some("sda1"));
    }

    #[test]
    fn parses_netlink_add() {
        let msg = b"add@/devices/pci0/usb1/1-1/1-1:1.0/ttyUSB0/tty/ttyUSB0\0ACTION=add\0MAJOR=188\0MINOR=0\0DEVNAME=ttyUSB0\0SUBSYSTEM=tty\0";
        let ev = parse_netlink_message(msg).expect("message");
        assert_eq!(ev.action, Some(Action::Add));
        assert!(ev.devpath.contains("ttyUSB0"));
        assert_eq!(ev.devname.as_deref(), Some("ttyUSB0"));
    }

    #[test]
    fn last_matching_rule_wins() {
        let rules = parse_conf_text(".* 0:0 600\nsd* 0:0 660\n");
        let rule = match_rule(&rules, "sda");
        assert_eq!(rule.mode, Mode::from_raw_mode(0o660));
    }
}
