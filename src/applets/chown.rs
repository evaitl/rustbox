use crate::sys;
use crate::{eprintln, usage};
use rustix::process::{Gid, Uid};

pub fn run(args: &[&str]) -> i32 {
    let mut recursive = false;
    let mut owner_str: Option<&str> = None;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-R" | "--recursive" => recursive = true,
            s if s.starts_with('-') => {
                usage("chown", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s if owner_str.is_none() => owner_str = Some(s),
            s => paths.push(s),
        }
    }

    let Some(owner_str) = owner_str else {
        usage("chown", "missing operand");
        return 1;
    };
    if paths.is_empty() {
        usage("chown", "missing operand after owner");
        return 1;
    }

    let (owner, group) = match parse_owner_group(owner_str) {
        Ok(ids) => ids,
        Err(()) => {
            usage("chown", &format!("invalid user: '{owner_str}'"));
            return 1;
        }
    };

    for path in paths {
        if let Err(e) = apply_owner(path, owner, group, recursive) {
            eprintln(format!("chown: cannot access '{path}': {e}"));
            return 1;
        }
    }
    0
}

fn apply_owner(
    path: &str,
    owner: Option<Uid>,
    group: Option<Gid>,
    recursive: bool,
) -> sys::Result<()> {
    sys::chown_path(path, owner, group)?;
    if recursive && sys::is_directory(path) {
        for entry in sys::read_dir(path)? {
            let child = join_path(path, &entry.name);
            apply_owner(&child, owner, group, true)?;
        }
    }
    Ok(())
}

fn parse_owner_group(s: &str) -> Result<(Option<Uid>, Option<Gid>), ()> {
    if let Some((user, group)) = s.split_once(':') {
        Ok((parse_uid(user), parse_gid(group)))
    } else {
        Ok((parse_uid(s), None))
    }
}

fn parse_uid(s: &str) -> Option<Uid> {
    if s.is_empty() {
        None
    } else {
        s.parse::<u32>().ok().map(Uid::from_raw)
    }
}

fn parse_gid(s: &str) -> Option<Gid> {
    if s.is_empty() {
        None
    } else {
        s.parse::<u32>().ok().map(Gid::from_raw)
    }
}

fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}
