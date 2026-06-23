use crate::sys;
use crate::{eprintln, usage};
use rustix::mount::UnmountFlags;

pub fn run(args: &[&str]) -> i32 {
    let mut all = false;
    let mut force = false;
    let mut lazy = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-a" => all = true,
            "-f" => force = true,
            "-l" => lazy = true,
            "-r" => {}
            s if s.starts_with('-') => {
                usage("umount", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => paths.push(s),
        }
    }

    let mut flags = UnmountFlags::empty();
    if force {
        flags |= UnmountFlags::FORCE;
    }
    if lazy {
        flags |= UnmountFlags::DETACH;
    }

    if all {
        return umount_all(flags);
    }

    if paths.is_empty() {
        usage("umount", "missing operand");
        return 1;
    }

    let mut status = 0;
    for path in paths {
        if let Err(e) = sys::unmount(path, flags) {
            eprintln(format!("umount: {path}: {e}"));
            status = 1;
        }
    }
    status
}

fn umount_all(flags: UnmountFlags) -> i32 {
    let table = match sys::read_mount_table() {
        Ok(table) => table,
        Err(e) => {
            eprintln(format!("umount: cannot read mount table: {e}"));
            return 1;
        }
    };

    let mut status = 0;
    for mountpoint in parse_mount_points(&table).into_iter().rev() {
        if mountpoint == "/" {
            continue;
        }
        if let Err(e) = sys::unmount(&mountpoint, flags) {
            eprintln(format!("umount: {mountpoint}: {e}"));
            status = 1;
        }
    }
    status
}

fn parse_mount_points(table: &str) -> Vec<String> {
    let mut points = Vec::new();
    for line in table.lines() {
        let mut fields = line.split_whitespace();
        let _device = fields.next();
        let Some(mountpoint) = fields.next() else {
            continue;
        };
        points.push(mountpoint.to_string());
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mount_points() {
        let table = "proc /proc proc rw 0 0\ntmpfs /run tmpfs rw 0 0\n";
        let points = parse_mount_points(table);
        assert_eq!(points, vec!["/proc", "/run"]);
    }
}
