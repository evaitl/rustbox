#![allow(dead_code)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rustbox"))
}

pub struct Rustbox {
    cmd: Command,
    cached_output: Option<Output>,
}

impl Rustbox {
    pub fn new() -> Self {
        Self {
            cmd: Command::new(bin()),
            cached_output: None,
        }
    }

    pub fn applet(mut self, name: &str) -> Self {
        self.cmd.arg(name);
        self
    }

    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.cmd.arg(arg);
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.cmd.args(args);
        self
    }

    pub fn current_dir(mut self, dir: &Path) -> Self {
        self.cmd.current_dir(dir);
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.cmd.env(key, value);
        self
    }

    pub fn stdin_input(mut self, input: &str) -> Self {
        use std::io::Write;
        use std::process::Stdio;
        self.cmd.stdin(Stdio::piped());
        let mut child = self.cmd.spawn().expect("failed to spawn rustbox");
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).expect("write stdin");
        }
        self.cached_output = Some(child.wait_with_output().expect("failed to run rustbox"));
        self
    }

    pub fn output(mut self) -> Output {
        if let Some(out) = self.cached_output.take() {
            out
        } else {
            self.cmd.output().expect("failed to run rustbox")
        }
    }

    pub fn status(self) -> i32 {
        self.output().status.code().unwrap_or(-1)
    }

    pub fn stdout(self) -> String {
        let out = self.output();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    pub fn stderr(self) -> String {
        let out = self.output();
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rustbox-test-{nanos}"));
        fs::create_dir_all(&path).expect("create test dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    pub fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, content).expect("write test file");
        path
    }

    pub fn read(&self, name: &str) -> String {
        fs::read_to_string(self.join(name)).expect("read test file")
    }

    pub fn exists(&self, name: &str) -> bool {
        self.join(name).exists()
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Linux `comm` names are limited to 15 bytes (16 with the terminating NUL).
#[cfg(target_os = "linux")]
pub fn unique_comm(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let suffix = (nanos % 10_000).to_string();
    let max_prefix = 15usize.saturating_sub(suffix.len());
    format!("{}{suffix}", &prefix[..prefix.len().min(max_prefix)])
}

#[cfg(target_os = "linux")]
pub fn wait_for_proc_comm(pid: u32, comm: &str) {
    use std::thread;
    use std::time::Duration;

    let comm_path = format!("/proc/{pid}/comm");
    for _ in 0..200 {
        if let Ok(name) = fs::read_to_string(&comm_path) {
            if name.trim() == comm {
                return;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("process {pid} with comm {comm:?} did not appear in /proc");
}

/// Spawn `sleep` with a unique `/proc/.../comm` name (not the generic `sleep`).
#[cfg(target_os = "linux")]
pub struct UniqueSleep {
    _dir: TestDir,
    pub child: std::process::Child,
}

#[cfg(target_os = "linux")]
impl UniqueSleep {
    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

#[cfg(target_os = "linux")]
fn locate_sleep() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("sleep"))
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("/usr/bin/sleep"))
}

#[cfg(target_os = "linux")]
pub fn spawn_sleep_with_comm(comm: &str, secs: u64) -> UniqueSleep {
    use std::os::unix::fs::symlink;
    use std::process::Stdio;

    assert!(
        comm.len() <= 15,
        "comm must fit in TASK_COMM_LEN (15 bytes): {comm:?}"
    );
    let dir = TestDir::new();
    let link = dir.path().join(comm);
    symlink(locate_sleep(), &link).expect("symlink sleep");
    let child = Command::new(&link)
        .arg(secs.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sleep");
    wait_for_proc_comm(child.id(), comm);
    UniqueSleep { _dir: dir, child }
}
