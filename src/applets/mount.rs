use crate::sys::{self, MountOptions};
use crate::{eprintln, usage};
use rustix::mount::MountFlags;

const FSTAB: &str = "/etc/fstab";

pub fn run(args: &[&str]) -> i32 {
    let mut all = false;
    let mut fstype: Option<&str> = None;
    let mut option_string: Option<&str> = None;
    let mut positions: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-a" => all = true,
            "-t" => {
                i += 1;
                if i >= args.len() {
                    usage("mount", "option requires an argument -- 't'");
                    return 1;
                }
                fstype = Some(args[i]);
            }
            "-o" => {
                i += 1;
                if i >= args.len() {
                    usage("mount", "option requires an argument -- 'o'");
                    return 1;
                }
                option_string = Some(args[i]);
            }
            "-r" => {
                option_string = Some("ro");
            }
            s if s.starts_with('-') => {
                usage("mount", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => positions.push(s),
        }
        i += 1;
    }

    if all {
        return mount_all();
    }

    if positions.is_empty() {
        if option_string.is_some() || fstype.is_some() {
            usage("mount", "missing mount point");
            return 1;
        }
        return list_mounts();
    }

    let opts = match option_string {
        Some(text) => match parse_options(text) {
            Ok(opts) => opts,
            Err(()) => return 1,
        },
        None => MountOptions::default(),
    };

    match positions.len() {
        1 => mount_one(None, positions[0], fstype, &opts),
        2 => mount_one(Some(positions[0]), positions[1], fstype, &opts),
        _ => {
            usage("mount", "too many arguments");
            1
        }
    }
}

fn list_mounts() -> i32 {
    match sys::read_mount_table() {
        Ok(table) => {
            print!("{table}");
            0
        }
        Err(e) => {
            eprintln(format!("mount: cannot read mount table: {e}"));
            1
        }
    }
}

fn mount_all() -> i32 {
    let text = match sys::read_to_string(FSTAB) {
        Ok(text) => text,
        Err(e) => {
            eprintln(format!("mount: cannot read '{FSTAB}': {e}"));
            return 1;
        }
    };

    let mut status = 0;
    for (line_no, line) in text.lines().enumerate() {
        let Some(entry) = parse_fstab_line(line) else {
            continue;
        };
        if entry.fstype == "swap" {
            continue;
        }
        let opts = match parse_options(&entry.options) {
            Ok(opts) => opts,
            Err(()) => {
                eprintln(format!("mount: {FSTAB}:{line_no}: invalid options"));
                status = 1;
                continue;
            }
        };
        if mount_one(
            Some(&entry.device),
            &entry.mountpoint,
            Some(&entry.fstype),
            &opts,
        ) != 0
        {
            status = 1;
        }
    }
    status
}

fn mount_one(source: Option<&str>, target: &str, fstype: Option<&str>, opts: &MountOptions) -> i32 {
    if let Err(e) = sys::apply_mount(source, target, fstype, opts) {
        let what = source.unwrap_or(target);
        eprintln(format!("mount: mounting {what} on {target} failed: {e}"));
        return 1;
    }
    0
}

struct FstabEntry {
    device: String,
    mountpoint: String,
    fstype: String,
    options: String,
}

fn parse_fstab_line(line: &str) -> Option<FstabEntry> {
    let line = line.split('#').next()?.trim();
    if line.is_empty() {
        return None;
    }
    let mut fields = line.split_whitespace();
    let device = fields.next()?.to_string();
    let mountpoint = fields.next()?.to_string();
    let fstype = fields.next()?.to_string();
    let options = fields.next().unwrap_or("defaults").to_string();
    Some(FstabEntry {
        device,
        mountpoint,
        fstype,
        options,
    })
}

fn parse_options(text: &str) -> Result<MountOptions, ()> {
    let mut opts = MountOptions::default();
    for part in text.split(',').filter(|p| !p.is_empty()) {
        match part {
            "defaults" | "_netdev" => {}
            "ro" => opts.flags |= MountFlags::RDONLY,
            "rw" => opts.flags.remove(MountFlags::RDONLY),
            "remount" => opts.remount = true,
            "bind" => opts.bind = true,
            "rbind" => {
                opts.bind = true;
                opts.rbind = true;
            }
            "nosuid" => opts.flags |= MountFlags::NOSUID,
            "nodev" => opts.flags |= MountFlags::NODEV,
            "noexec" => opts.flags |= MountFlags::NOEXEC,
            "noatime" => opts.flags |= MountFlags::NOATIME,
            "nodiratime" => opts.flags |= MountFlags::NODIRATIME,
            "relatime" => opts.flags |= MountFlags::RELATIME,
            "strictatime" => opts.flags |= MountFlags::STRICTATIME,
            "dirsync" => opts.flags |= MountFlags::DIRSYNC,
            "lazytime" => opts.flags |= MountFlags::LAZYTIME,
            "sync" => opts.flags |= MountFlags::SYNCHRONOUS,
            "rec" => opts.flags |= MountFlags::REC,
            s if s.contains('=') => opts.data = Some(s.to_string()),
            s => {
                usage("mount", &format!("unknown option '{s}'"));
                return Err(());
            }
        }
    }
    Ok(opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_options() {
        let opts = parse_options("ro,remount").unwrap();
        assert!(opts.flags.contains(MountFlags::RDONLY));
        assert!(opts.remount);
    }

    #[test]
    fn parses_bind_recursive() {
        let opts = parse_options("bind,rec").unwrap();
        assert!(opts.bind);
        assert!(opts.flags.contains(MountFlags::REC));
    }

    #[test]
    fn parses_fstab_line() {
        let entry = parse_fstab_line("UUID=abc / ext4 defaults 0 1").unwrap();
        assert_eq!(entry.device, "UUID=abc");
        assert_eq!(entry.mountpoint, "/");
        assert_eq!(entry.fstype, "ext4");
    }
}
