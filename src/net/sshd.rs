//! SSH server: password auth, PTY-backed rash shell.

use crate::passwd_auth::{self, AuthPasswdTable};
use crate::passwd_lookup;
use crate::sys::{self, Error};
use russh::keys::{encode_pkcs8_pem, load_secret_key, Algorithm, PrivateKey, PublicKey};
use russh::server::{Auth, Config as RusshConfig, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId};
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

pub const DEFAULT_CONFIG: &str = "/etc/sshd.conf";
pub const DEFAULT_PASSWD: &str = passwd_lookup::DEFAULT_PASSWD;
pub const DEFAULT_HOSTKEY: &str = "/etc/sshd_host_key";

const AUTH_FAIL_LIMIT: u32 = 3;
const AUTH_FAIL_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub listen_addr: String,
    pub port: u16,
    pub passwd_path: String,
    pub hostkey_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0".to_string(),
            port: 22,
            passwd_path: DEFAULT_PASSWD.to_string(),
            hostkey_path: DEFAULT_HOSTKEY.to_string(),
        }
    }
}

pub type PasswdTable = AuthPasswdTable;

pub fn load_passwd(path: &str) -> PasswdTable {
    passwd_auth::load_auth_passwd(path)
}

pub fn parse_passwd_text(text: &str) -> PasswdTable {
    passwd_auth::parse_auth_passwd_text(text)
}

/// Tracks failed password attempts per client IP.
#[derive(Clone, Debug)]
struct AuthRateLimiter {
    limit: u32,
    window: Duration,
    failures: HashMap<String, Vec<Instant>>,
}

impl Default for AuthRateLimiter {
    fn default() -> Self {
        Self::new(AUTH_FAIL_LIMIT, AUTH_FAIL_WINDOW)
    }
}

impl AuthRateLimiter {
    fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            failures: HashMap::new(),
        }
    }

    fn peer_key(peer: Option<SocketAddr>) -> String {
        peer.map(|addr| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn is_blocked(&mut self, key: &str) -> bool {
        self.prune(key);
        self.failures
            .get(key)
            .is_some_and(|times| times.len() >= self.limit as usize)
    }

    fn record_failure(&mut self, key: &str) {
        self.prune(key);
        self.failures
            .entry(key.to_string())
            .or_default()
            .push(Instant::now());
    }

    fn clear(&mut self, key: &str) {
        self.failures.remove(key);
    }

    fn prune(&mut self, key: &str) {
        let now = Instant::now();
        if let Some(times) = self.failures.get_mut(key) {
            times.retain(|t| now.duration_since(*t) < self.window);
            if times.is_empty() {
                self.failures.remove(key);
            }
        }
    }
}

pub fn load_config(path: &str) -> sys::Result<Config> {
    let text = sys::read_to_string(path)?;
    Ok(parse_config_text(&text))
}

pub fn parse_config_text(text: &str) -> Config {
    let mut cfg = Config::default();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        match key.to_ascii_lowercase().as_str() {
            "listen" => {
                if let Some(value) = parts.next() {
                    cfg.listen_addr = value.to_string();
                }
            }
            "port" => {
                if let Some(value) = parts.next() {
                    if let Ok(port) = value.parse::<u16>() {
                        cfg.port = port;
                    }
                }
            }
            "passwd" => {
                if let Some(value) = parts.next() {
                    cfg.passwd_path = value.to_string();
                }
            }
            "hostkey" => {
                if let Some(value) = parts.next() {
                    cfg.hostkey_path = value.to_string();
                }
            }
            _ => {}
        }
    }
    cfg
}

pub async fn serve(cfg: Config) -> std::io::Result<()> {
    let host_key = load_or_create_host_key(&cfg.hostkey_path)?;
    let passwd = Arc::new(load_passwd(&cfg.passwd_path));
    let ssh_config = Arc::new(RusshConfig {
        auth_rejection_time: Duration::from_secs(1),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        keys: vec![host_key],
        ..Default::default()
    });

    let addr = format!("{}:{}", cfg.listen_addr, cfg.port);
    let mut server = SshServer {
        passwd,
        rate_limiter: Arc::new(Mutex::new(AuthRateLimiter::default())),
    };
    server.run_on_address(ssh_config, &addr).await
}

fn load_or_create_host_key(path: &str) -> sys::Result<PrivateKey> {
    if Path::new(path).exists() {
        return load_secret_key(path, None).map_err(|_| Error::IO);
    }
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = sys::mkdir_all(parent.to_str().unwrap_or("/etc"));
        }
    }
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).map_err(|_| Error::IO)?;
    let mut file = std::fs::File::create(path).map_err(|_| Error::IO)?;
    encode_pkcs8_pem(&key, &mut file).map_err(|_| Error::IO)?;
    Ok(key)
}

struct SshServer {
    passwd: Arc<PasswdTable>,
    rate_limiter: Arc<Mutex<AuthRateLimiter>>,
}

impl Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, peer: Option<SocketAddr>) -> Self::Handler {
        SshSession::new(
            Arc::clone(&self.passwd),
            Arc::clone(&self.rate_limiter),
            peer,
        )
    }
}

struct ChannelState {
    pty_write: Mutex<std::fs::File>,
    child_pid: rustix::process::Pid,
}

struct SshSession {
    passwd: Arc<PasswdTable>,
    rate_limiter: Arc<Mutex<AuthRateLimiter>>,
    peer: Option<SocketAddr>,
    pty_sizes: Mutex<HashMap<ChannelId, (u32, u32)>>,
    channels: Arc<Mutex<HashMap<ChannelId, ChannelState>>>,
}

impl SshSession {
    fn new(
        passwd: Arc<PasswdTable>,
        rate_limiter: Arc<Mutex<AuthRateLimiter>>,
        peer: Option<SocketAddr>,
    ) -> Self {
        Self {
            passwd,
            rate_limiter,
            peer,
            pty_sizes: Mutex::new(HashMap::new()),
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Handler for SshSession {
    type Error = anyhow::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> std::result::Result<Auth, Self::Error> {
        let key = AuthRateLimiter::peer_key(self.peer);
        {
            let mut limiter = self.rate_limiter.lock().unwrap();
            if limiter.is_blocked(&key) {
                return Ok(Auth::reject());
            }
            if self.passwd.check(user, password) {
                limiter.clear(&key);
                return Ok(Auth::Accept);
            }
            limiter.record_failure(&key);
        }
        Ok(Auth::reject())
    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _public_key: &PublicKey,
    ) -> std::result::Result<Auth, Self::Error> {
        Ok(Auth::reject())
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        self.pty_sizes
            .lock()
            .unwrap()
            .insert(channel, (col_width, row_height));
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        let size = self
            .pty_sizes
            .lock()
            .unwrap()
            .get(&channel)
            .copied()
            .unwrap_or((80, 24));
        match spawn_pty_shell(size) {
            Ok((master_read, pty_write, child_pid)) => {
                let handle = session.handle();
                let channels = Arc::clone(&self.channels);
                tokio::spawn(async move {
                    let mut reader = master_read;
                    let mut buf = [0u8; 4096];
                    loop {
                        match reader.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                let chunk = buf[..n].to_vec();
                                if handle.data(channel, chunk).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    let _ = handle.eof(channel).await;
                    let _ = handle.close(channel).await;
                    channels.lock().unwrap().remove(&channel);
                });
                self.channels.lock().unwrap().insert(
                    channel,
                    ChannelState {
                        pty_write: Mutex::new(pty_write),
                        child_pid,
                    },
                );
                session.channel_success(channel)?;
            }
            Err(_) => {
                session.channel_failure(channel)?;
            }
        }
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(state) = self.channels.lock().unwrap().get(&channel) {
            state
                .pty_write
                .lock()
                .unwrap()
                .write_all(data)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(state) = self.channels.lock().unwrap().get(&channel) {
            let _ = state.pty_write.lock().unwrap().flush();
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(state) = self.channels.lock().unwrap().remove(&channel) {
            let _ = rustix::process::kill_process(state.child_pid, rustix::process::Signal::TERM);
            let _ = rustix::process::waitpid(
                Some(state.child_pid),
                rustix::process::WaitOptions::NOHANG,
            );
        }
        self.pty_sizes.lock().unwrap().remove(&channel);
        Ok(())
    }
}

fn spawn_pty_shell(
    (cols, rows): (u32, u32),
) -> sys::Result<(tokio::fs::File, std::fs::File, rustix::process::Pid)> {
    let (master, slave) = openpty()?;
    set_winsize(slave, cols, rows)?;

    match unsafe { rustix::runtime::kernel_fork() }.map_err(|_| Error::IO)? {
        rustix::runtime::Fork::Child(_) => {
            let _ = close_fd(master);
            if unsafe { libc::setsid() } < 0 {
                rustix::runtime::exit_group(1);
            }
            if unsafe { libc::ioctl(slave, libc::TIOCSCTTY, 0) } < 0 {
                rustix::runtime::exit_group(1);
            }
            for fd in [0, 1, 2] {
                if unsafe { libc::dup2(slave, fd) } < 0 {
                    rustix::runtime::exit_group(1);
                }
            }
            if slave > 2 {
                let _ = close_fd(slave);
            }
            let exe =
                std::fs::read_link("/proc/self/exe").unwrap_or_else(|_| PathBuf::from("rustbox"));
            let exe = exe.to_string_lossy();
            if sys::exec_argv(&[&exe, "rash"]).is_err() {
                let _ = sys::exec_argv(&["rustbox", "rash"]);
            }
            rustix::runtime::exit_group(127);
        }
        rustix::runtime::Fork::ParentOf(pid) => {
            let _ = close_fd(slave);
            let read_file = unsafe { std::fs::File::from_raw_fd(master) };
            let write_file = read_file.try_clone().map_err(|_| Error::IO)?;
            let master_read = tokio::fs::File::from_std(read_file);
            Ok((master_read, write_file, pid))
        }
    }
}

fn openpty() -> sys::Result<(libc::c_int, libc::c_int)> {
    let mut master: libc::c_int = 0;
    let mut slave: libc::c_int = 0;
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } < 0
    {
        return Err(sys::last_errno());
    }
    Ok((master, slave))
}

fn set_winsize(fd: libc::c_int, cols: u32, rows: u32) -> sys::Result<()> {
    let ws = libc::winsize {
        ws_row: rows as libc::c_ushort,
        ws_col: cols as libc::c_ushort,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    if unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) } < 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}

fn close_fd(fd: libc::c_int) -> sys::Result<()> {
    if unsafe { libc::close(fd) } < 0 {
        Err(sys::last_errno())
    } else {
        Ok(())
    }
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
        let _ = parse_config_text(text);
        let table = parse_passwd_text(text);
        let _ = table.check("user", "pass");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_bcrypt_passwd_entry() {
        let hash = bcrypt::hash("rustbox", 4).expect("hash");
        let table = parse_passwd_text(&format!("rustbox:{hash}:0:0:RustBox:/root:/bin/rash"));
        assert!(table.check("rustbox", "rustbox"));
        assert!(!table.check("rustbox", "wrong"));
        assert!(!table.check("other", "rustbox"));
    }

    #[test]
    fn rejects_plaintext_passwd_entries() {
        let table = parse_passwd_text("rustbox:rustbox:0:0:RustBox:/root:/bin/rash\n");
        assert!(!table.check("rustbox", "rustbox"));
    }

    #[test]
    fn missing_passwd_file_yields_empty_table() {
        let table = load_passwd("/nonexistent/passwd");
        assert!(table.is_empty());
        assert!(!table.check("rustbox", "rustbox"));
    }

    #[test]
    fn parses_passwd_file_hashes() {
        let hash = bcrypt::hash("secret", 4).expect("hash");
        let table = parse_passwd_text(&format!(
            "admin:{hash}:0:0:Admin:/root:/bin/rash\n# comment\n"
        ));
        assert!(table.check("admin", "secret"));
    }

    #[test]
    fn parses_config() {
        let cfg = parse_config_text("port 2222\npasswd /tmp/pw\n");
        assert_eq!(cfg.port, 2222);
        assert_eq!(cfg.passwd_path, "/tmp/pw");
    }

    #[test]
    #[ignore = "prints bcrypt hash: cargo test --features applet-sshd --lib -- --ignored --nocapture hash_passwd_line"]
    fn hash_passwd_line() {
        let pass = std::env::var("SSHD_HASH_PASS").unwrap_or_else(|_| "rustbox".to_string());
        let hash = bcrypt::hash(pass, 12).expect("hash");
        println!("sshd-passwd: {hash}");
    }

    #[test]
    fn initrd_template_passwd_accepts_root() {
        let text = include_str!("../../initrd/template/etc/passwd");
        let table = parse_passwd_text(text);
        assert!(table.check("root", "rustbox"));
    }

    #[test]
    fn rate_limiter_blocks_after_three_failures() {
        let mut limiter = AuthRateLimiter::new(3, Duration::from_secs(60));
        let key = "127.0.0.1";
        assert!(!limiter.is_blocked(key));
        limiter.record_failure(key);
        limiter.record_failure(key);
        assert!(!limiter.is_blocked(key));
        limiter.record_failure(key);
        assert!(limiter.is_blocked(key));
    }

    #[test]
    fn rate_limiter_clears_on_success() {
        let mut limiter = AuthRateLimiter::new(3, Duration::from_secs(60));
        let key = "10.0.0.1";
        limiter.record_failure(key);
        limiter.record_failure(key);
        limiter.clear(key);
        limiter.record_failure(key);
        limiter.record_failure(key);
        assert!(!limiter.is_blocked(key));
    }

    #[test]
    fn rate_limiter_expires_old_failures() {
        use std::thread;
        let mut limiter = AuthRateLimiter::new(3, Duration::from_millis(50));
        let key = "192.168.1.1";
        for _ in 0..3 {
            limiter.record_failure(key);
        }
        assert!(limiter.is_blocked(key));
        thread::sleep(Duration::from_millis(60));
        assert!(!limiter.is_blocked(key));
    }
}
