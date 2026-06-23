use crate::sys;
use crate::{eprintln, usage};

#[cfg(target_os = "linux")]
use rustix::fd::OwnedFd;
#[cfg(target_os = "linux")]
use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, StatFs};
#[cfg(target_os = "linux")]
use rustix::mount::UnmountFlags;
#[cfg(target_os = "linux")]
use rustix::process;
#[cfg(target_os = "linux")]
use rustix::runtime::{self, Fork};

const MOVE_MOUNTS: &[&str] = &["/dev", "/proc", "/sys", "/run"];

#[cfg(target_os = "linux")]
const TMPFS_MAGIC: u64 = 0x0102_1994;
#[cfg(target_os = "linux")]
const RAMFS_MAGIC: u64 = 0x8584_58f6;

pub fn run(args: &[&str]) -> i32 {
    #[cfg(not(target_os = "linux"))]
    {
        eprintln("switch_root: not supported on this platform");
        return 1;
    }

    #[cfg(target_os = "linux")]
    {
        if args.len() < 2 {
            usage("switch_root", "not enough arguments");
            return 1;
        }

        let newroot = args[0];
        if newroot.is_empty() || args[1].is_empty() {
            usage("switch_root", "bad usage");
            return 1;
        }

        if let Err(e) = switch_to(newroot) {
            eprintln(format!("switch_root: {e}"));
            return 1;
        }

        match sys::exec_argv(&args[1..]) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("switch_root: cannot execute '{}': {e}", args[1]));
                1
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn switch_to(newroot: &str) -> sys::Result<()> {
    let oldroot_stat = fs::stat("/")?;
    let newroot_stat = fs::stat(newroot)?;

    for mountpoint in MOVE_MOUNTS {
        move_mount_if_needed(mountpoint, newroot, &oldroot_stat, &newroot_stat)?;
    }

    process::chdir(newroot)?;

    let oldroot_fd = fs::open("/", OFlags::RDONLY.union(OFlags::DIRECTORY), Mode::empty())?;

    rustix::mount::mount_move(newroot, "/")?;

    process::chroot(".")?;
    process::chdir("/")?;

    match unsafe { runtime::kernel_fork()? } {
        Fork::Child(_) => {
            cleanup_old_root(oldroot_fd);
            runtime::exit_group(0);
        }
        Fork::ParentOf(_) => Ok(()),
    }
}

#[cfg(target_os = "linux")]
fn move_mount_if_needed(
    mountpoint: &str,
    newroot: &str,
    oldroot_stat: &rustix::fs::Stat,
    newroot_stat: &rustix::fs::Stat,
) -> sys::Result<()> {
    let newmount = join_path(newroot, mountpoint);

    if fs::stat(mountpoint).is_ok_and(|st| st.st_dev == oldroot_stat.st_dev) {
        return Ok(());
    }

    if fs::stat(&newmount).is_err()
        || fs::stat(&newmount).is_ok_and(|st| st.st_dev != newroot_stat.st_dev)
    {
        let _ = sys::unmount(mountpoint, UnmountFlags::DETACH);
        return Ok(());
    }

    if let Err(e) = rustix::mount::mount_move(mountpoint, &newmount) {
        eprintln(format!(
            "switch_root: failed to move {mountpoint} to {newmount}: {e}"
        ));
        eprintln(format!("switch_root: forcing unmount of {mountpoint}"));
        let _ = sys::unmount(mountpoint, UnmountFlags::FORCE);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn cleanup_old_root(oldroot_fd: OwnedFd) {
    let Ok(st) = fs::fstatfs(&oldroot_fd) else {
        return;
    };
    if !is_initramfs(&st) {
        eprintln("switch_root: old root filesystem is not an initramfs");
        return;
    }
    let Ok(root_stat) = fs::fstat(&oldroot_fd) else {
        return;
    };
    if let Err(e) = recursive_remove_dir(oldroot_fd, root_stat.st_dev) {
        eprintln(format!("switch_root: failed to clean old root: {e}"));
    }
}

#[cfg(target_os = "linux")]
fn is_initramfs(st: &StatFs) -> bool {
    let magic = st.f_type as u64;
    magic == TMPFS_MAGIC || magic == RAMFS_MAGIC
}

#[cfg(target_os = "linux")]
fn recursive_remove_dir(fd: OwnedFd, root_dev: u64) -> sys::Result<()> {
    let mut dir = Dir::new(fd)?;
    let mut names = Vec::new();
    for entry in &mut dir {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy();
        if name == "." || name == ".." {
            continue;
        }
        names.push(name.into_owned());
    }

    let dir_fd = dir.fd()?;
    for name in names {
        let st = fs::statat(dir_fd, &name, AtFlags::SYMLINK_NOFOLLOW)?;
        if st.st_dev as u64 != root_dev {
            continue;
        }

        if FileType::from_raw_mode(st.st_mode).is_dir() {
            let child = fs::openat(
                dir_fd,
                &name,
                OFlags::RDONLY
                    .union(OFlags::DIRECTORY)
                    .union(OFlags::CLOEXEC),
                Mode::empty(),
            )?;
            recursive_remove_dir(child, root_dev)?;
            fs::unlinkat(dir_fd, &name, AtFlags::REMOVEDIR)?;
        } else {
            fs::unlinkat(dir_fd, &name, AtFlags::empty())?;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn join_path(base: &str, suffix: &str) -> String {
    format!("{base}{suffix}")
}
