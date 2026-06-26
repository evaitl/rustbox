//! Thin wrappers around [`rustix`] for applet use.

use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{self, Dir, FileType, Mode, OFlags, Stat, Timespec};
use rustix::fs::{Nsecs, Secs};
use rustix::io::{self, read, write};
use rustix::process::getcwd;
use rustix::stdio;
use rustix::thread::{self, NanosleepRelativeResult};

pub type Error = io::Errno;
pub type Result<T> = io::Result<T>;

pub fn last_errno() -> Error {
    Error::from_raw_os_error(unsafe { *libc::__errno_location() })
}

const READ_FLAGS: OFlags = OFlags::RDONLY.union(OFlags::CLOEXEC);
const DIR_FLAGS: OFlags = READ_FLAGS.union(OFlags::DIRECTORY);
const CREATE_FLAGS: OFlags = OFlags::WRONLY
    .union(OFlags::CREATE)
    .union(OFlags::CLOEXEC)
    .union(OFlags::TRUNC);

pub fn open_read(path: &str) -> Result<OwnedFd> {
    fs::open(path, READ_FLAGS, Mode::empty())
}

pub fn open_create(path: &str) -> Result<OwnedFd> {
    fs::open(path, CREATE_FLAGS, Mode::RWXU)
}

pub fn stat(path: &str) -> Result<Stat> {
    fs::stat(path)
}

pub fn lstat(path: &str) -> Result<Stat> {
    fs::lstat(path)
}

pub fn chmod_path(path: &str, mode: Mode) -> Result<()> {
    fs::chmod(path, mode)
}

pub fn chown_path(
    path: &str,
    owner: Option<rustix::process::Uid>,
    group: Option<rustix::process::Gid>,
) -> Result<()> {
    fs::chown(path, owner, group)
}

pub fn check_access(path: &str, access: rustix::fs::Access) -> bool {
    fs::access(path, access).is_ok()
}

pub fn file_type(path: &str) -> Result<FileType> {
    Ok(FileType::from_raw_mode(stat(path)?.st_mode))
}

pub fn is_directory(path: &str) -> bool {
    file_type(path).is_ok_and(|t| t.is_dir())
}

pub fn exists(path: &str) -> bool {
    stat(path).is_ok()
}

pub fn read_to_end<Fd: AsFd>(fd: Fd) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = read(fd.as_fd(), &mut buf)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

pub fn read_to_string(path: &str) -> Result<String> {
    let fd = open_read(path)?;
    let bytes = read_to_end(fd)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn copy_fd_to_stdout<Fd: AsFd>(fd: Fd) -> Result<()> {
    copy_fd_to_fd(fd, stdio::stdout())
}

pub fn copy_fd_to_fd<FdIn: AsFd, FdOut: AsFd>(input: FdIn, output: FdOut) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::sendfile;
        let mut offset = 0u64;
        loop {
            match sendfile(output.as_fd(), input.as_fd(), Some(&mut offset), 64 * 1024) {
                Ok(0) => return Ok(()),
                Ok(_) => {}
                Err(io::Errno::INTR) => {}
                Err(e) => return Err(e),
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut buf = [0u8; 8192];
        loop {
            let n = read(input.as_fd(), &mut buf)?;
            if n == 0 {
                return Ok(());
            }
            let mut off = 0;
            while off < n {
                off += write(output.as_fd(), &buf[off..n])?;
            }
        }
    }
}

pub fn copy_file(src: &str, dst: &str) -> Result<()> {
    let src_fd = open_read(src)?;
    let dst_fd = open_create(dst)?;
    copy_fd_to_fd(src_fd, dst_fd)
}

pub fn mkdir_one(path: &str) -> Result<()> {
    fs::mkdir(path, Mode::RWXU)
}

pub fn mkfifo(path: &str, mode: Mode) -> Result<()> {
    fs::mkfifoat(fs::CWD, path, mode)
}

pub fn mknod(path: &str, file_type: FileType, mode: Mode, major: u32, minor: u32) -> Result<()> {
    let dev = makedev(major, minor);
    fs::mknodat(fs::CWD, path, file_type, mode, dev)
}

fn makedev(major: u32, minor: u32) -> fs::Dev {
    libc::makedev(major, minor) as fs::Dev
}

pub fn mkdir_all(path: &str) -> Result<()> {
    if exists(path) {
        return Ok(());
    }
    if let Some(parent) = parent_path(path) {
        mkdir_all(parent)?;
    }
    mkdir_one(path)
}

pub fn remove_file(path: &str) -> Result<()> {
    fs::unlink(path)
}

pub fn remove_dir(path: &str) -> Result<()> {
    fs::rmdir(path)
}

pub fn remove_dir_all(path: &str) -> Result<()> {
    let fd = fs::open(path, DIR_FLAGS, Mode::empty())?;
    let mut dir = Dir::new(fd)?;
    let mut children = Vec::new();
    for entry in &mut dir {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy();
        if name == "." || name == ".." {
            continue;
        }
        children.push(name.into_owned());
    }
    for child in children {
        let child_path = join_path(path, &child);
        if is_directory(&child_path) {
            remove_dir_all(&child_path)?;
        } else {
            remove_file(&child_path)?;
        }
    }
    remove_dir(path)
}

pub fn remove_path(path: &str, recursive: bool) -> Result<()> {
    let kind = file_type(path)?;
    if kind.is_dir() {
        if recursive {
            remove_dir_all(path)
        } else {
            remove_dir(path)
        }
    } else {
        remove_file(path)
    }
}

pub fn rename_path(old: &str, new: &str) -> Result<()> {
    fs::rename(old, new)
}

pub fn hard_link(old: &str, new: &str) -> Result<()> {
    fs::link(old, new)
}

pub fn sym_link(target: &str, link: &str) -> Result<()> {
    fs::symlink(target, link)
}

pub fn read_link(path: &str) -> Result<String> {
    let buf = fs::readlink(path, Vec::new())?;
    Ok(buf.to_string_lossy().into_owned())
}

pub fn current_dir() -> Result<String> {
    let cwd = getcwd(Vec::new())?;
    Ok(cwd.to_string_lossy().into_owned())
}

pub fn sleep_seconds(seconds: f64) -> Result<()> {
    let mut request = Timespec {
        tv_sec: seconds.trunc() as Secs,
        tv_nsec: (seconds.fract() * 1_000_000_000.0) as Nsecs,
    };
    loop {
        match thread::nanosleep(&request) {
            NanosleepRelativeResult::Ok => return Ok(()),
            NanosleepRelativeResult::Interrupted(remain) => request = remain,
            NanosleepRelativeResult::Err(e) => return Err(e),
        }
    }
}

pub struct DirEntryInfo {
    pub name: String,
    pub file_type: FileType,
}

pub fn read_dir(path: &str) -> Result<Vec<DirEntryInfo>> {
    let fd = fs::open(path, DIR_FLAGS, Mode::empty())?;
    let mut dir = Dir::new(fd)?;
    let mut entries = Vec::new();
    for entry in &mut dir {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy();
        if name == "." || name == ".." {
            continue;
        }
        entries.push(DirEntryInfo {
            name: name.into_owned(),
            file_type: entry.file_type(),
        });
    }
    entries.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    Ok(entries)
}

pub fn copy_dir_recursive(src: &str, dst: &str) -> Result<()> {
    mkdir_all(dst)?;
    for entry in read_dir(src)? {
        let src_child = join_path(src, &entry.name);
        let dst_child = join_path(dst, &entry.name);
        if entry.file_type.is_dir() {
            copy_dir_recursive(&src_child, &dst_child)?;
        } else {
            copy_file(&src_child, &dst_child)?;
        }
    }
    Ok(())
}

pub fn for_each_line<Fd: AsFd>(fd: Fd, mut f: impl FnMut(&[u8]) -> bool) -> Result<()> {
    let mut carry = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = read(fd.as_fd(), &mut buf)?;
        if n == 0 {
            if !carry.is_empty() && !f(&carry) {
                return Ok(());
            }
            break;
        }
        let mut start = 0;
        for (i, &byte) in buf[..n].iter().enumerate() {
            if byte == b'\n' {
                carry.extend_from_slice(&buf[start..i]);
                if !f(&carry) {
                    return Ok(());
                }
                carry.clear();
                start = i + 1;
            }
        }
        carry.extend_from_slice(&buf[start..n]);
    }
    Ok(())
}

pub fn mode_string(st: &Stat) -> String {
    let ft = FileType::from_raw_mode(st.st_mode);
    let kind = if ft.is_dir() {
        'd'
    } else if ft.is_symlink() {
        'l'
    } else {
        '-'
    };
    let bits = st.st_mode & 0o7777;
    let mut out = String::with_capacity(10);
    out.push(kind);
    for shift in [6, 3, 0] {
        let trio = (bits >> shift) & 0o7;
        out.push(if trio & 4 != 0 { 'r' } else { '-' });
        out.push(if trio & 2 != 0 { 'w' } else { '-' });
        out.push(if trio & 1 != 0 { 'x' } else { '-' });
    }
    out
}

pub fn format_mtime(st: &Stat) -> String {
    let secs = st.st_mtime.max(0) as u64;
    let days = secs / 86_400;
    let hour = (secs % 86_400) / 3_600;
    let minute = (secs % 3_600) / 60;
    format!("{days:>3} {hour:02}:{minute:02}")
}

fn parent_path(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    path.rfind('/')
        .map(|idx| &path[..idx])
        .filter(|p| !p.is_empty())
}

fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

pub fn is_init_process() -> bool {
    rustix::process::getpid() == rustix::process::Pid::INIT
}

pub fn spawn(command: &str) -> Result<rustix::process::Pid> {
    let mut parts = command.split_whitespace();
    let program = parts.next().ok_or(io::Errno::INVAL)?;
    let args: Vec<&str> = parts.collect();
    spawn_argv(program, &args)
}

pub fn spawn_respawn(command: &str) -> Result<rustix::process::Pid> {
    spawn_respawn_on(command, "/dev/console")
}

pub fn spawn_respawn_on(command: &str, device: &str) -> Result<rustix::process::Pid> {
    let mut parts = command.split_whitespace();
    let program = parts.next().ok_or(io::Errno::INVAL)?;
    let args: Vec<&str> = parts.collect();
    spawn_respawn_argv_on(program, &args, device)
}

pub fn dup2_stdin(fd: &impl AsFd) -> Result<()> {
    rustix::stdio::dup2_stdin(fd).map_err(|_| io::Errno::IO)
}

pub fn dup2_stdout(fd: &impl AsFd) -> Result<()> {
    rustix::stdio::dup2_stdout(fd).map_err(|_| io::Errno::IO)
}

pub fn dup2_stderr(fd: &impl AsFd) -> Result<()> {
    rustix::stdio::dup2_stderr(fd).map_err(|_| io::Errno::IO)
}

/// Reattach stdin, stdout, and stderr to `/dev/null`.
fn redirect_stdio_to_dev_null() -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let devnull = open_read("/dev/null")?;
    let fd = devnull.as_raw_fd();
    dup2_stdin(&devnull)?;
    dup2_stdout(&devnull)?;
    dup2_stderr(&devnull)?;
    if fd > 2 {
        let _ = unsafe { libc::close(fd) };
    }
    Ok(())
}

/// Reopen stdio on a TTY device (getty-style: open, dup, setsid, controlling tty).
pub fn reopen_stdio_to_device(path: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;

    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(path)
        .map_err(|_| io::Errno::IO)?;
    let fd = tty.as_raw_fd();
    dup2_stdin(&tty)?;
    dup2_stdout(&tty)?;
    dup2_stderr(&tty)?;
    if fd > 2 {
        let _ = unsafe { libc::close(fd) };
    }
    let _ = rustix::process::setsid();
    let _ = unsafe { libc::ioctl(0, libc::TIOCSCTTY, 1) };
    Ok(())
}

/// Reopen the system console on stdio.
pub fn reopen_stdio_to_console() -> Result<()> {
    reopen_stdio_to_device("/dev/console")
}

pub fn exec_argv(argv: &[&str]) -> Result<()> {
    if argv.is_empty() {
        return Err(io::Errno::INVAL);
    }
    let program = argv[0];
    let prog = std::ffi::CString::new(program).map_err(|_| io::Errno::INVAL)?;
    let mut c_args = Vec::with_capacity(argv.len());
    for arg in argv {
        c_args.push(std::ffi::CString::new(*arg).map_err(|_| io::Errno::INVAL)?);
    }
    let mut arg_ptrs: Vec<*const u8> = c_args.iter().map(|s| s.as_ptr().cast()).collect();
    arg_ptrs.push(std::ptr::null());

    let env: Vec<std::ffi::CString> = std::env::vars_os()
        .map(|(key, value)| {
            use std::os::unix::ffi::OsStrExt;
            let mut entry = Vec::with_capacity(key.len() + value.len() + 1);
            entry.extend_from_slice(key.as_bytes());
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            std::ffi::CString::new(entry).map_err(|_| io::Errno::INVAL)
        })
        .collect::<Result<_>>()?;
    let mut env_ptrs: Vec<*const u8> = env.iter().map(|s| s.as_ptr().cast()).collect();
    env_ptrs.push(std::ptr::null());

    unsafe {
        let err = rustix::runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr());
        Err(err)
    }
}

pub fn spawn_argv(program: &str, args: &[&str]) -> Result<rustix::process::Pid> {
    let prog = std::ffi::CString::new(program).map_err(|_| io::Errno::INVAL)?;
    let mut argv = vec![prog.clone()];
    for arg in args {
        argv.push(std::ffi::CString::new(*arg).map_err(|_| io::Errno::INVAL)?);
    }
    let mut arg_ptrs: Vec<*const u8> = argv.iter().map(|s| s.as_ptr().cast()).collect();
    arg_ptrs.push(std::ptr::null());

    let env: Vec<std::ffi::CString> = std::env::vars_os()
        .map(|(key, value)| {
            use std::os::unix::ffi::OsStrExt;
            let mut entry = Vec::with_capacity(key.len() + value.len() + 1);
            entry.extend_from_slice(key.as_bytes());
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            std::ffi::CString::new(entry).map_err(|_| io::Errno::INVAL)
        })
        .collect::<Result<_>>()?;
    let mut env_ptrs: Vec<*const u8> = env.iter().map(|s| s.as_ptr().cast()).collect();
    env_ptrs.push(std::ptr::null());

    unsafe {
        match rustix::runtime::kernel_fork()? {
            rustix::runtime::Fork::Child(_) => {
                let _ =
                    rustix::runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr());
                rustix::runtime::exit_group(127);
            }
            rustix::runtime::Fork::ParentOf(pid) => Ok(pid),
        }
    }
}

pub fn spawn_respawn_argv(program: &str, args: &[&str]) -> Result<rustix::process::Pid> {
    spawn_respawn_argv_on(program, args, "/dev/console")
}

pub fn spawn_respawn_argv_on(
    program: &str,
    args: &[&str],
    device: &str,
) -> Result<rustix::process::Pid> {
    let prog = std::ffi::CString::new(program).map_err(|_| io::Errno::INVAL)?;
    let mut argv = vec![prog.clone()];
    for arg in args {
        argv.push(std::ffi::CString::new(*arg).map_err(|_| io::Errno::INVAL)?);
    }
    let mut arg_ptrs: Vec<*const u8> = argv.iter().map(|s| s.as_ptr().cast()).collect();
    arg_ptrs.push(std::ptr::null());

    let env: Vec<std::ffi::CString> = std::env::vars_os()
        .map(|(key, value)| {
            use std::os::unix::ffi::OsStrExt;
            let mut entry = Vec::with_capacity(key.len() + value.len() + 1);
            entry.extend_from_slice(key.as_bytes());
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            std::ffi::CString::new(entry).map_err(|_| io::Errno::INVAL)
        })
        .collect::<Result<_>>()?;
    let mut env_ptrs: Vec<*const u8> = env.iter().map(|s| s.as_ptr().cast()).collect();
    env_ptrs.push(std::ptr::null());

    unsafe {
        match rustix::runtime::kernel_fork()? {
            rustix::runtime::Fork::Child(_) => {
                let _ = reopen_stdio_to_device(device);
                let _ =
                    rustix::runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr());
                rustix::runtime::exit_group(127);
            }
            rustix::runtime::Fork::ParentOf(pid) => Ok(pid),
        }
    }
}

pub fn wait_pid(pid: rustix::process::Pid) -> Result<i32> {
    loop {
        match rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::empty())? {
            Some((_, status)) => return Ok(exit_status(status)),
            None => continue,
        }
    }
}

pub fn wait_any() -> Result<Option<(rustix::process::Pid, i32)>> {
    match rustix::process::waitpid(None, rustix::process::WaitOptions::empty()) {
        Ok(Some((pid, status))) => Ok(Some((pid, exit_status(status)))),
        Ok(None) => Ok(None),
        Err(io::Errno::CHILD) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn reap_zombies() -> Result<Vec<(rustix::process::Pid, i32)>> {
    let mut reaped = Vec::new();
    while let Some((pid, status)) =
        rustix::process::waitpid(None, rustix::process::WaitOptions::NOHANG)?
    {
        reaped.push((pid, exit_status(status)));
    }
    Ok(reaped)
}

fn exit_status(status: rustix::process::WaitStatus) -> i32 {
    if let Some(code) = status.exit_status() {
        code
    } else if let Some(sig) = status.terminating_signal() {
        128 + sig
    } else {
        1
    }
}

#[derive(Clone, Debug)]
pub struct MountOptions {
    pub flags: rustix::mount::MountFlags,
    pub remount: bool,
    pub bind: bool,
    pub rbind: bool,
    pub data: Option<String>,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            flags: rustix::mount::MountFlags::empty(),
            remount: false,
            bind: false,
            rbind: false,
            data: None,
        }
    }
}

pub fn read_mount_table() -> Result<String> {
    read_to_string("/proc/mounts")
}

pub fn apply_mount(
    source: Option<&str>,
    target: &str,
    fstype: Option<&str>,
    opts: &MountOptions,
) -> Result<()> {
    use rustix::mount;

    if opts.remount {
        let data = opts.data.as_deref().unwrap_or("");
        return mount::mount_remount(target, opts.flags, data);
    }

    if opts.bind {
        let source = source.ok_or(io::Errno::INVAL)?;
        if opts.rbind {
            return mount::mount_bind_recursive(source, target);
        }
        return mount::mount_bind(source, target);
    }

    let source = source.unwrap_or("none");
    let fstype = fstype.unwrap_or("auto");
    let data = opts
        .data
        .as_ref()
        .map(|d| std::ffi::CString::new(d.as_str()))
        .transpose()
        .map_err(|_| io::Errno::INVAL)?;
    let data_ref = data.as_deref();
    mount::mount(source, target, fstype, opts.flags, data_ref)
}

pub fn unmount(target: &str, flags: rustix::mount::UnmountFlags) -> Result<()> {
    rustix::mount::unmount(target, flags)
}

#[cfg(target_os = "linux")]
pub fn sync_filesystems() {
    rustix::fs::sync();
}

#[cfg(target_os = "linux")]
const SYSLOG_ACTION_READ_ALL: i32 = 3;
#[cfg(target_os = "linux")]
const SYSLOG_ACTION_CLEAR: i32 = 5;

#[cfg(target_os = "linux")]
pub fn read_kernel_log(buf: &mut [u8]) -> Result<usize> {
    let n = unsafe {
        libc::syscall(
            libc::SYS_syslog,
            SYSLOG_ACTION_READ_ALL,
            buf.as_mut_ptr(),
            buf.len(),
        )
    };
    if n < 0 {
        Err(io::Errno::from_raw_os_error((-n) as i32))
    } else {
        Ok(n as usize)
    }
}

#[cfg(not(target_os = "linux"))]
pub fn read_kernel_log(_buf: &mut [u8]) -> Result<usize> {
    Err(io::Errno::NOTSUP)
}

#[cfg(target_os = "linux")]
pub fn clear_kernel_log() -> Result<()> {
    let ret = unsafe { libc::syscall(libc::SYS_syslog, SYSLOG_ACTION_CLEAR, 0, 0) };
    if ret < 0 {
        Err(io::Errno::from_raw_os_error((-ret) as i32))
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
pub fn clear_kernel_log() -> Result<()> {
    Err(io::Errno::NOTSUP)
}

#[cfg(target_os = "linux")]
pub fn daemonize() -> Result<()> {
    use rustix::fs::Mode as ModeBits;

    unsafe {
        match rustix::runtime::kernel_fork()? {
            rustix::runtime::Fork::ParentOf(_) => rustix::runtime::exit_group(0),
            rustix::runtime::Fork::Child(_) => {}
        }

        if rustix::process::setsid().is_err() {
            rustix::runtime::exit_group(1);
        }

        match rustix::runtime::kernel_fork()? {
            rustix::runtime::Fork::ParentOf(_) => rustix::runtime::exit_group(0),
            rustix::runtime::Fork::Child(_) => {}
        }
    }

    if rustix::process::chdir("/").is_err() {
        return Err(io::Errno::IO);
    }
    let _ = rustix::process::umask(ModeBits::from_raw_mode(0));
    redirect_stdio_to_dev_null()?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn daemonize() -> Result<()> {
    Err(io::Errno::NOTSUP)
}

#[cfg(target_os = "linux")]
pub fn set_clock_realtime(secs: i64, nsecs: i64) -> Result<()> {
    rustix::time::clock_settime(
        rustix::time::ClockId::Realtime,
        Timespec {
            tv_sec: secs,
            tv_nsec: nsecs,
        },
    )
}

#[cfg(not(target_os = "linux"))]
pub fn set_clock_realtime(_secs: i64, _nsecs: i64) -> Result<()> {
    Err(io::Errno::NOTSUP)
}

pub fn kill_pid(pid: u32, sig: rustix::process::Signal) -> Result<()> {
    let pid = rustix::process::Pid::from_raw(pid as i32).ok_or(io::Errno::INVAL)?;
    rustix::process::kill_process(pid, sig)
}

pub fn test_kill_pid(pid: u32) -> Result<()> {
    let pid = rustix::process::Pid::from_raw(pid as i32).ok_or(io::Errno::INVAL)?;
    rustix::process::test_kill_process(pid)
}

pub struct ProcInfo {
    pub pid: u32,
    pub comm: String,
}

pub fn list_processes() -> Result<Vec<ProcInfo>> {
    let mut procs = Vec::new();
    for entry in read_dir("/proc")? {
        let Ok(pid) = entry.name.parse::<u32>() else {
            continue;
        };
        let comm_path = format!("/proc/{pid}/comm");
        let comm = read_to_string(&comm_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "?".to_string());
        procs.push(ProcInfo { pid, comm });
    }
    procs.sort_by_key(|p| p.pid);
    Ok(procs)
}

pub struct MemInfo {
    pub mem_total_kb: u64,
    pub mem_free_kb: u64,
    pub mem_available_kb: u64,
    pub buffers_kb: u64,
    pub cached_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
}

pub fn read_meminfo() -> Result<MemInfo> {
    let text = read_to_string("/proc/meminfo")?;
    let mut info = MemInfo {
        mem_total_kb: 0,
        mem_free_kb: 0,
        mem_available_kb: 0,
        buffers_kb: 0,
        cached_kb: 0,
        swap_total_kb: 0,
        swap_free_kb: 0,
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let kb = value
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        match key {
            "MemTotal" => info.mem_total_kb = kb,
            "MemFree" => info.mem_free_kb = kb,
            "MemAvailable" => info.mem_available_kb = kb,
            "Buffers" => info.buffers_kb = kb,
            "Cached" => info.cached_kb = kb,
            "SwapTotal" => info.swap_total_kb = kb,
            "SwapFree" => info.swap_free_kb = kb,
            _ => {}
        }
    }
    Ok(info)
}

pub struct UptimeInfo {
    pub uptime_secs: f64,
    pub load_1: f64,
    pub load_5: f64,
    pub load_15: f64,
}

pub fn read_uptime() -> Result<UptimeInfo> {
    let uptime_text = read_to_string("/proc/uptime")?;
    let mut parts = uptime_text.split_whitespace();
    let uptime_secs: f64 = parts
        .next()
        .ok_or(io::Errno::IO)?
        .parse()
        .map_err(|_| io::Errno::IO)?;
    let load_text = read_to_string("/proc/loadavg")?;
    let mut load = load_text.split_whitespace();
    let load_1: f64 = load.next().unwrap_or("0").parse().unwrap_or(0.0);
    let load_5: f64 = load.next().unwrap_or("0").parse().unwrap_or(0.0);
    let load_15: f64 = load.next().unwrap_or("0").parse().unwrap_or(0.0);
    Ok(UptimeInfo {
        uptime_secs,
        load_1,
        load_5,
        load_15,
    })
}

pub fn proc_rss_kb(pid: u32) -> Option<u64> {
    let status = read_to_string(&format!("/proc/{pid}/status")).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next().and_then(|s| s.parse().ok());
        }
    }
    None
}

pub fn sync_all() {
    rustix::fs::sync();
}

pub fn sysctl_proc_path(key: &str) -> String {
    format!("/proc/sys/{}", key.replace('.', "/"))
}

pub fn read_sysctl(key: &str) -> Result<String> {
    read_to_string(&sysctl_proc_path(key)).map(|s| s.trim().to_string())
}

pub fn write_sysctl(key: &str, value: &str) -> Result<()> {
    let fd = open_create(&sysctl_proc_path(key))?;
    write(&fd, value.as_bytes())?;
    write(&fd, b"\n")?;
    Ok(())
}

pub fn file_size(path: &str) -> Result<u64> {
    Ok(stat(path)?.st_size as u64)
}

pub fn write_file(path: &str, data: &[u8]) -> Result<()> {
    use rustix::fs::{open, Mode, OFlags};
    let fd = open(
        path,
        OFlags::WRONLY | OFlags::CREATE | OFlags::TRUNC,
        Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::WGRP,
    )?;
    write(&fd, data)?;
    Ok(())
}

pub fn append_file(path: &str, data: &[u8]) -> Result<()> {
    use rustix::fs::{open, Mode, OFlags};
    let fd = open(
        path,
        OFlags::WRONLY | OFlags::APPEND | OFlags::CREATE,
        Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::WGRP,
    )?;
    write(&fd, data)?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn swapon(path: &str, priority: Option<i32>) -> Result<()> {
    let cpath = std::ffi::CString::new(path).map_err(|_| io::Errno::INVAL)?;
    let flags = match priority {
        Some(prio) => {
            const SWAP_FLAG_PREFER: libc::c_int = 0x8000;
            const SWAP_FLAG_PRIO_MASK: libc::c_int = 0x7fff;
            let prio = prio.clamp(0, SWAP_FLAG_PRIO_MASK);
            SWAP_FLAG_PREFER | prio
        }
        None => 0,
    };
    let ret = unsafe { libc::swapon(cpath.as_ptr(), flags) };
    if ret < 0 {
        return Err(last_errno());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn swapoff(path: &str) -> Result<()> {
    let cpath = std::ffi::CString::new(path).map_err(|_| io::Errno::INVAL)?;
    let ret = unsafe { libc::swapoff(cpath.as_ptr()) };
    if ret < 0 {
        return Err(last_errno());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn list_swap_devices() -> Result<Vec<String>> {
    let text = read_to_string("/proc/swaps")?;
    let mut devices = Vec::new();
    for line in text.lines().skip(1) {
        let filename = line.split_whitespace().next().unwrap_or("");
        if !filename.is_empty() {
            devices.push(filename.to_string());
        }
    }
    Ok(devices)
}

#[cfg(target_os = "linux")]
pub fn walk_sysctl(prefix: &str, out: &mut Vec<(String, String)>) -> Result<()> {
    let path = if prefix.is_empty() {
        "/proc/sys".to_string()
    } else {
        sysctl_proc_path(prefix)
    };
    if !is_directory(&path) {
        if let Ok(value) = read_to_string(&path) {
            out.push((prefix.to_string(), value.trim().to_string()));
        }
        return Ok(());
    }
    for entry in read_dir(&path)? {
        let name = entry.name;
        if name == "." || name == ".." {
            continue;
        }
        let key = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}.{name}")
        };
        walk_sysctl(&key, out)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn system_reboot(cmd: rustix::system::RebootCommand) -> Result<()> {
    rustix::system::reboot(cmd)
}
