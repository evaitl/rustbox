//! Minimal HTTP server with CGI/1.1 support.

use crate::eprintln;
use crate::net::http_client;
use crate::sys::{self, Error, Result};
use rustix::fd::{AsRawFd, BorrowedFd, RawFd};
use rustix::io::{self, read, write};
use rustix::pipe::pipe;
use rustix::process::Signal;
use rustix::runtime::{self, Fork};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_CONFIG: &str = "/etc/thttpd.conf";
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const SERVER_SOFTWARE: &str = "rustbox-thttpd/0.1";
const SMOKE_TEST_PORT: u16 = 18080;
const SMOKE_CGI_PATH: &str = "/cgi-bin/smoke-cgi";
const SMOKE_CGI_MARKER: &[u8] = b"smoke-cgi-ok";
const SMOKE_LISTING_PATH: &str = "/listing-test/";
const SMOKE_LISTING_MARKER: &[u8] = b"listing-sample";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    pub dir: String,
    pub cgidir: String,
    pub user: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 80,
            dir: "/var/www".to_string(),
            cgidir: "/var/www/cgi-bin".to_string(),
            user: "http".to_string(),
        }
    }
}

pub fn default_config_path() -> &'static str {
    DEFAULT_CONFIG
}

pub fn load_config(path: &str) -> Result<Config> {
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
        let (key, value) = match line.split_once('=') {
            Some(pair) => pair,
            None => match line.split_once(char::is_whitespace) {
                Some(pair) => pair,
                None => continue,
            },
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "port" => {
                if let Ok(port) = value.parse::<u16>() {
                    cfg.port = port;
                }
            }
            "dir" | "root" | "documentroot" => cfg.dir = value.to_string(),
            "cgidir" | "cgi" => cfg.cgidir = value.to_string(),
            "user" => cfg.user = value.to_string(),
            _ => {}
        }
    }
    cfg
}

pub fn serve(cfg: Config) -> Result<()> {
    let listen_fd = listen_socket(cfg.port)?;
    drop_daemon_privileges(&cfg.user)?;
    loop {
        let _ = sys::reap_zombies();
        let client = match accept_connection(listen_fd) {
            Ok(fd) => fd,
            Err(Error::INTR) => continue,
            Err(e) => return Err(e),
        };
        match unsafe { runtime::kernel_fork() } {
            Ok(Fork::Child(_)) => {
                close_fd(listen_fd);
                let status = if handle_client(client, &cfg).is_ok() {
                    0
                } else {
                    1
                };
                close_fd(client);
                runtime::exit_group(status);
            }
            Ok(Fork::ParentOf(_)) => close_fd(client),
            Err(_) => close_fd(client),
        }
    }
}

pub fn smoke_test(mut cfg: Config) -> Result<()> {
    cfg.port = SMOKE_TEST_PORT;
    let script = join_path(&cfg.cgidir, "smoke-cgi");
    if !sys::exists(&script) {
        return Err(Error::NOENT);
    }

    match unsafe { runtime::kernel_fork() }? {
        Fork::Child(_) => {
            let _ = serve(cfg);
            runtime::exit_group(1);
        }
        Fork::ParentOf(server_pid) => {
            let mut body = Err(Error::AGAIN);
            for _ in 0..100 {
                sys::sleep_seconds(0.1)?;
                match http_client::http_get("127.0.0.1", SMOKE_TEST_PORT, SMOKE_CGI_PATH) {
                    Ok(response) => {
                        body = Ok(response);
                        break;
                    }
                    Err(Error::AGAIN) | Err(Error::CONNREFUSED) | Err(Error::IO) => {}
                    Err(e) => {
                        let _ = stop_smoke_server(server_pid);
                        return Err(e);
                    }
                }
            }
            let body = body.map_err(|_| Error::TIMEDOUT)?;
            if !body
                .windows(SMOKE_CGI_MARKER.len())
                .any(|window| window == SMOKE_CGI_MARKER)
            {
                let _ = stop_smoke_server(server_pid);
                return Err(Error::IO);
            }

            let listing = match http_get_with_retry(SMOKE_TEST_PORT, SMOKE_LISTING_PATH) {
                Ok(listing) => listing,
                Err(e) => {
                    let _ = stop_smoke_server(server_pid);
                    return Err(e);
                }
            };
            if let Err(e) = wget_smoke(SMOKE_TEST_PORT, SMOKE_CGI_PATH, SMOKE_CGI_MARKER) {
                let _ = stop_smoke_server(server_pid);
                return Err(e);
            }
            if let Err(e) = wget_smoke(SMOKE_TEST_PORT, SMOKE_LISTING_PATH, SMOKE_LISTING_MARKER) {
                let _ = stop_smoke_server(server_pid);
                return Err(e);
            }
            let _ = stop_smoke_server(server_pid);
            if listing
                .windows(SMOKE_LISTING_MARKER.len())
                .any(|window| window == SMOKE_LISTING_MARKER)
            {
                Ok(())
            } else {
                Err(Error::IO)
            }
        }
    }
}

fn http_get_with_retry(port: u16, path: &str) -> Result<Vec<u8>> {
    let mut body = Err(Error::AGAIN);
    for _ in 0..100 {
        sys::sleep_seconds(0.1)?;
        match http_client::http_get("127.0.0.1", port, path) {
            Ok(response) => {
                body = Ok(response);
                break;
            }
            Err(Error::AGAIN) | Err(Error::CONNREFUSED) | Err(Error::IO) => {}
            Err(e) => return Err(e),
        }
    }
    body.map_err(|_| Error::TIMEDOUT)
}

fn wget_smoke(port: u16, path: &str, marker: &[u8]) -> Result<()> {
    const OUT: &str = "/tmp/thttpd-smoke-wget.out";
    let _ = sys::remove_file(OUT);
    let url = format!("http://127.0.0.1:{port}{path}");
    let pid = sys::spawn_argv("/bin/wget", &["-q", "-O", OUT, &url])?;
    let status = sys::wait_pid(pid).unwrap_or(1);
    if status != 0 {
        return Err(Error::IO);
    }
    let body = sys::read_to_string(OUT)?;
    if body
        .as_bytes()
        .windows(marker.len())
        .any(|window| window == marker)
    {
        Ok(())
    } else {
        Err(Error::IO)
    }
}

fn stop_smoke_server(pid: rustix::process::Pid) -> Result<()> {
    let _ = sys::kill_pid(pid.as_raw_nonzero().get() as u32, Signal::KILL);
    let _ = sys::wait_pid(pid);
    let _ = sys::reap_zombies();
    Ok(())
}

fn close_fd(fd: RawFd) {
    unsafe {
        io::close(fd);
    }
}

fn drop_daemon_privileges(user: &str) -> Result<()> {
    crate::passwd_lookup::drop_to_user(user).map_err(|err| {
        eprintln(format!("thttpd: privilege drop failed: {err}"));
        Error::PERM
    })
}

fn listen_socket(port: u16) -> Result<RawFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(sys::last_errno());
    }
    let yes: libc::c_int = 1;
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &yes as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }
    let addr = libc::sockaddr_in {
        sin_family: libc::AF_INET as _,
        sin_port: port.to_be(),
        sin_addr: libc::in_addr { s_addr: 0 },
        sin_zero: [0; 8],
    };
    if unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        unsafe {
            io::close(fd);
        }
        return Err(err);
    }
    if unsafe { libc::listen(fd, 8) } < 0 {
        let err = sys::last_errno();
        unsafe {
            io::close(fd);
        }
        return Err(err);
    }
    Ok(fd)
}

fn accept_connection(listen_fd: RawFd) -> Result<RawFd> {
    let fd = unsafe { libc::accept(listen_fd, std::ptr::null_mut(), std::ptr::null_mut()) };
    if fd < 0 {
        return Err(sys::last_errno());
    }
    Ok(fd)
}

struct Request {
    method: String,
    path: String,
    query: String,
    version: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn handle_client(client: RawFd, cfg: &Config) -> Result<()> {
    let req = match read_request(client)? {
        Some(req) => req,
        None => return Ok(()),
    };
    match req.method.as_str() {
        "GET" | "HEAD" => dispatch_get(client, cfg, &req),
        "POST" => dispatch_post(client, cfg, &req),
        _ => send_error(client, 501, "Not Implemented"),
    }
}

fn read_request(client: RawFd) -> Result<Option<Request>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if buf.len() >= MAX_HEADER_BYTES {
            return Ok(None);
        }
        let n = read(unsafe { BorrowedFd::borrow_raw(client) }, &mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = find_header_end(&buf) {
            let header_bytes = &buf[..end];
            let body_start = skip_header_terminator(&buf, end);
            let mut body = buf[body_start..].to_vec();
            let req = parse_request_headers(header_bytes)?;
            let content_length = req
                .headers
                .get("content-length")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
                .min(MAX_BODY_BYTES);
            while body.len() < content_length {
                let n = read(unsafe { BorrowedFd::borrow_raw(client) }, &mut chunk)?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..n]);
                if body.len() > MAX_BODY_BYTES {
                    return Ok(None);
                }
            }
            body.truncate(content_length);
            return Ok(Some(Request { body, ..req }));
        }
        if buf.len() >= MAX_HEADER_BYTES {
            return Ok(None);
        }
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| buf.windows(2).position(|w| w == b"\n\n").map(|i| i + 2))
}

fn skip_header_terminator(_buf: &[u8], end: usize) -> usize {
    end
}

fn parse_request_headers(header_bytes: &[u8]) -> Result<Request> {
    let text = String::from_utf8_lossy(header_bytes);
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("").trim();
    if request_line.is_empty() {
        return Err(Error::INVAL);
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let version = parts.next().unwrap_or("HTTP/1.0").to_string();
    let (path, query) = split_target(&target);
    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    Ok(Request {
        method,
        path,
        query,
        version,
        headers,
        body: Vec::new(),
    })
}

fn split_target(target: &str) -> (String, String) {
    let target = target.split_whitespace().next().unwrap_or("/");
    match target.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (target.to_string(), String::new()),
    }
}

fn dispatch_get(client: RawFd, cfg: &Config, req: &Request) -> Result<()> {
    if let Some(script) = cgi_script_path(cfg, &req.path) {
        return run_cgi(client, cfg, req, &script);
    }
    serve_static(client, cfg, req, false)
}

fn dispatch_post(client: RawFd, cfg: &Config, req: &Request) -> Result<()> {
    if let Some(script) = cgi_script_path(cfg, &req.path) {
        return run_cgi(client, cfg, req, &script);
    }
    send_error(client, 405, "Method Not Allowed")
}

fn cgi_script_path(cfg: &Config, url_path: &str) -> Option<String> {
    let prefix = cgi_url_prefix(cfg);
    let rest = url_path.strip_prefix(&prefix)?;
    if rest.is_empty() || rest.contains("..") {
        return None;
    }
    let script = join_path(&cfg.cgidir, rest.trim_start_matches('/'));
    if sys::exists(&script) && sys::check_access(&script, rustix::fs::Access::EXEC_OK) {
        Some(script)
    } else {
        None
    }
}

fn cgi_url_prefix(cfg: &Config) -> String {
    if cfg.cgidir.starts_with(&cfg.dir) {
        let rel = cfg
            .cgidir
            .strip_prefix(&cfg.dir)
            .unwrap_or(&cfg.cgidir)
            .trim_start_matches('/');
        if rel.is_empty() {
            return "/cgi-bin/".to_string();
        }
        return format!("/{rel}/");
    }
    "/cgi-bin/".to_string()
}

fn serve_static(client: RawFd, cfg: &Config, req: &Request, _post: bool) -> Result<()> {
    let mut fs_path = map_url_to_file(cfg, &req.path)?;
    let mut list_dir: Option<String> = None;

    if sys::is_directory(&fs_path) {
        list_dir = Some(fs_path.clone());
        fs_path = join_path(&fs_path, "index.html");
    } else if is_index_request(&req.path) {
        list_dir = parent_dir(&fs_path);
    }

    if !sys::exists(&fs_path) {
        if fs_path.ends_with("index.html") {
            if let Some(dir) = list_dir.or_else(|| Some(cfg.dir.clone())) {
                let listing = directory_listing(&dir)?;
                let head_only = req.method == "HEAD";
                return send_response(
                    client,
                    200,
                    "OK",
                    "text/plain; charset=utf-8",
                    &listing,
                    head_only,
                );
            }
        }
        return send_error(client, 404, "Not Found");
    }
    if sys::is_directory(&fs_path) {
        return send_error(client, 403, "Forbidden");
    }
    let fd = sys::open_read(&fs_path)?;
    let body = sys::read_to_end(fd)?;
    let content_type = guess_content_type(&fs_path);
    let head_only = req.method == "HEAD";
    send_response(client, 200, "OK", &content_type, &body, head_only)
}

fn is_index_request(path: &str) -> bool {
    let p = path.trim_start_matches('/');
    p.is_empty() || p == "index.html" || p.ends_with("/index.html") || path.ends_with('/')
}

fn parent_dir(path: &str) -> Option<String> {
    let parent = Path::new(path).parent()?;
    let s = parent.to_str()?;
    if s.is_empty() {
        Some("/".to_string())
    } else {
        Some(s.to_string())
    }
}

fn directory_listing(dir: &str) -> Result<Vec<u8>> {
    let (out_read, out_write) = pipe()?;
    match unsafe { runtime::kernel_fork() }? {
        Fork::Child(_) => {
            unsafe {
                let _ = rustix::stdio::dup2_stdout(&out_write);
                io::close(out_read.as_raw_fd());
                io::close(out_write.as_raw_fd());
            }
            let prog = match std::ffi::CString::new("/bin/ls") {
                Ok(p) => p,
                Err(_) => runtime::exit_group(127),
            };
            let arg_al = match std::ffi::CString::new("-al") {
                Ok(p) => p,
                Err(_) => runtime::exit_group(127),
            };
            let arg_dir = match std::ffi::CString::new(dir) {
                Ok(p) => p,
                Err(_) => runtime::exit_group(127),
            };
            let arg_ptrs = [
                prog.as_ptr().cast(),
                arg_al.as_ptr().cast(),
                arg_dir.as_ptr().cast(),
                std::ptr::null(),
            ];
            let c_env: Vec<std::ffi::CString> = std::env::vars()
                .filter_map(|(k, v)| std::ffi::CString::new(format!("{k}={v}")).ok())
                .collect();
            let mut env_ptrs: Vec<*const u8> = c_env.iter().map(|s| s.as_ptr().cast()).collect();
            env_ptrs.push(std::ptr::null());
            let _ =
                unsafe { runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr()) };
            runtime::exit_group(127);
        }
        Fork::ParentOf(pid) => {
            drop(out_write);
            let output = sys::read_to_end(out_read)?;
            let status = sys::wait_pid(pid).unwrap_or(1);
            if status != 0 && output.is_empty() {
                return Err(Error::IO);
            }
            Ok(output)
        }
    }
}

fn map_url_to_file(cfg: &Config, url_path: &str) -> Result<String> {
    let mut path = url_path.trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }
    if path.contains("..") || path.contains('\0') {
        return Err(Error::PERM);
    }
    for part in path.split('/') {
        if part == ".." {
            return Err(Error::PERM);
        }
    }
    Ok(join_path(&cfg.dir, &path))
}

fn guess_content_type(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        "text/html; charset=utf-8".to_string()
    } else if lower.ends_with(".txt") {
        "text/plain; charset=utf-8".to_string()
    } else if lower.ends_with(".css") {
        "text/css".to_string()
    } else if lower.ends_with(".js") {
        "application/javascript".to_string()
    } else if lower.ends_with(".json") {
        "application/json".to_string()
    } else if lower.ends_with(".png") {
        "image/png".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

fn run_cgi(client: RawFd, cfg: &Config, req: &Request, script: &str) -> Result<()> {
    let prefix = cgi_url_prefix(cfg);
    let path_info = req
        .path
        .strip_prefix(prefix.trim_end_matches('/'))
        .unwrap_or("")
        .to_string();
    let content_type = req.headers.get("content-type").cloned().unwrap_or_default();
    let content_length = req.body.len().to_string();
    let remote_addr = "127.0.0.1".to_string();
    let server_name = "localhost".to_string();
    let server_port = cfg.port.to_string();
    let script_name = req.path.clone();

    let mut env: Vec<(String, String)> = std::env::vars().collect();
    env.push(("REQUEST_METHOD".into(), req.method.clone()));
    env.push(("QUERY_STRING".into(), req.query.clone()));
    env.push(("CONTENT_LENGTH".into(), content_length));
    env.push(("CONTENT_TYPE".into(), content_type));
    env.push(("PATH_INFO".into(), path_info));
    env.push(("SCRIPT_NAME".into(), script_name));
    env.push(("SERVER_SOFTWARE".into(), SERVER_SOFTWARE.to_string()));
    env.push(("SERVER_PROTOCOL".into(), req.version.clone()));
    env.push(("GATEWAY_INTERFACE".into(), "CGI/1.1".into()));
    env.push(("SERVER_NAME".into(), server_name));
    env.push(("SERVER_PORT".into(), server_port));
    env.push(("REMOTE_ADDR".into(), remote_addr));
    env.push(("DOCUMENT_ROOT".into(), cfg.dir.clone()));

    let output = exec_cgi(script, &req.body, &env)?;
    write_cgi_response(client, &output)
}

struct CgiOutput {
    status: u16,
    reason: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn exec_cgi(script: &str, body: &[u8], env: &[(String, String)]) -> Result<CgiOutput> {
    let (body_read, body_write) = pipe()?;
    let (out_read, out_write) = pipe()?;
    match unsafe { runtime::kernel_fork() }? {
        Fork::Child(_) => {
            unsafe {
                let _ = rustix::stdio::dup2_stdin(&body_read);
                let _ = rustix::stdio::dup2_stdout(&out_write);
                io::close(body_read.as_raw_fd());
                io::close(body_write.as_raw_fd());
                io::close(out_read.as_raw_fd());
                io::close(out_write.as_raw_fd());
            }
            let prog = match std::ffi::CString::new(script) {
                Ok(p) => p,
                Err(_) => runtime::exit_group(127),
            };
            let c_env: Vec<std::ffi::CString> = env
                .iter()
                .filter_map(|(k, v)| std::ffi::CString::new(format!("{k}={v}")).ok())
                .collect();
            let mut env_ptrs: Vec<*const u8> = c_env.iter().map(|s| s.as_ptr().cast()).collect();
            env_ptrs.push(std::ptr::null());
            let arg_ptrs = [prog.as_ptr().cast(), std::ptr::null()];
            let _ =
                unsafe { runtime::execve(prog.as_c_str(), arg_ptrs.as_ptr(), env_ptrs.as_ptr()) };
            runtime::exit_group(127);
        }
        Fork::ParentOf(pid) => {
            drop(body_read);
            drop(out_write);
            if !body.is_empty() {
                let _ = write(&body_write, body);
            }
            drop(body_write);
            let output = sys::read_to_end(out_read)?;
            let status = sys::wait_pid(pid).unwrap_or(1);
            if status != 0 && output.is_empty() {
                return Err(Error::IO);
            }
            Ok(parse_cgi_output(&output))
        }
    }
}

fn parse_cgi_output(raw: &[u8]) -> CgiOutput {
    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        let header_text = String::from_utf8_lossy(&raw[..pos]);
        return split_cgi_headers(&header_text, &raw[pos + 4..]);
    }
    if let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") {
        let header_text = String::from_utf8_lossy(&raw[..pos]);
        return split_cgi_headers(&header_text, &raw[pos + 2..]);
    }
    CgiOutput {
        status: 200,
        reason: "OK".to_string(),
        headers: vec![("Content-Type".into(), "text/html".into())],
        body: raw.to_vec(),
    }
}

fn split_cgi_headers(header_text: &str, body: &[u8]) -> CgiOutput {
    let mut status = 200u16;
    let mut reason = "OK".to_string();
    let mut headers = Vec::new();
    for line in header_text.lines() {
        if let Some(rest) = line.strip_prefix("Status:") {
            let rest = rest.trim();
            if let Some((code, msg)) = rest.split_once(char::is_whitespace) {
                status = code.parse().unwrap_or(200);
                reason = msg.to_string();
            } else if let Ok(code) = rest.parse::<u16>() {
                status = code;
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    if headers.is_empty() {
        headers.push(("Content-Type".into(), "text/html".into()));
    }
    CgiOutput {
        status,
        reason,
        headers,
        body: body.to_vec(),
    }
}

fn write_cgi_response(client: RawFd, output: &CgiOutput) -> Result<()> {
    let mut response = format!("HTTP/1.0 {} {}\r\n", output.status, output.reason);
    for (name, value) in &output.headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str("\r\n");
    write_all(client, response.as_bytes())?;
    write_all(client, &output.body)?;
    Ok(())
}

fn send_error(client: RawFd, code: u16, reason: &str) -> Result<()> {
    let body = format!("<html><head><title>{code} {reason}</title></head><body><h1>{code} {reason}</h1></body></html>");
    send_response(
        client,
        code,
        reason,
        "text/html; charset=utf-8",
        body.as_bytes(),
        false,
    )
}

fn send_response(
    client: RawFd,
    code: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> Result<()> {
    let date = http_date();
    let header = format!(
        "HTTP/1.0 {code} {reason}\r\nDate: {date}\r\nServer: {SERVER_SOFTWARE}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    write_all(client, header.as_bytes())?;
    if !head_only {
        write_all(client, body)?;
    }
    Ok(())
}

fn write_all(fd: RawFd, mut data: &[u8]) -> Result<()> {
    while !data.is_empty() {
        match write(unsafe { BorrowedFd::borrow_raw(fd) }, data) {
            Ok(0) => return Err(Error::IO),
            Ok(n) => data = &data[n..],
            Err(Error::INTR) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn http_date() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn join_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
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
    }

    let mut buf = data.to_vec();
    if find_header_end(&buf).is_none() && buf.len() + 4 <= MAX_HEADER_BYTES {
        buf.extend_from_slice(b"\r\n\r\n");
    }
    if buf.len() <= MAX_HEADER_BYTES {
        let _ = parse_request_headers(&buf);
    }

    let _ = parse_cgi_output(data);

    let cfg = Config::default();
    if let Ok(text) = std::str::from_utf8(data) {
        for line in text.lines().take(8) {
            let path = line.split_whitespace().nth(1).unwrap_or(line);
            let _ = map_url_to_file(&cfg, path);
            let _ = cgi_script_path(&cfg, path);
            let _ = guess_content_type(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config_file() {
        let cfg = parse_config_text(
            "# comment\nport=8080\ndir=/srv/www\ncgidir=/srv/www/cgi-bin\nuser=www\n",
        );
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.dir, "/srv/www");
        assert_eq!(cfg.cgidir, "/srv/www/cgi-bin");
        assert_eq!(cfg.user, "www");
    }

    #[test]
    fn default_user_is_http() {
        assert_eq!(Config::default().user, "http");
    }

    #[test]
    fn rejects_path_traversal() {
        let cfg = Config::default();
        assert!(map_url_to_file(&cfg, "/../etc/passwd").is_err());
    }
}
