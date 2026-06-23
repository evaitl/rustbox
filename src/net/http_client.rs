//! Minimal HTTP/1.0 client (GET only).

use crate::net::ipv4;
use crate::sys::{self, Error, Result};
use rustix::fd::{BorrowedFd, RawFd};
use rustix::io::{self, read, write};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum HttpScheme {
    Http,
    #[cfg(feature = "wget-tls")]
    Https,
}

#[derive(Debug, PartialEq, Eq)]
pub struct HttpUrl {
    pub scheme: HttpScheme,
    pub host: String,
    pub port: u16,
    pub path: String,
}

#[cfg(feature = "wget-tls")]
pub fn parse_url(url: &str) -> Option<HttpUrl> {
    parse_http_url(url).or(parse_https_url(url))
}

#[cfg(not(feature = "wget-tls"))]
pub fn parse_url(url: &str) -> Option<HttpUrl> {
    parse_http_url(url)
}

pub fn parse_http_url(url: &str) -> Option<HttpUrl> {
    parse_url_with_scheme(url, "http://", HttpScheme::Http, 80)
}

#[cfg(feature = "wget-tls")]
pub fn parse_https_url(url: &str) -> Option<HttpUrl> {
    parse_url_with_scheme(url, "https://", HttpScheme::Https, 443)
}

fn parse_url_with_scheme(
    url: &str,
    prefix: &str,
    scheme: HttpScheme,
    default_port: u16,
) -> Option<HttpUrl> {
    let rest = url.strip_prefix(prefix)?;
    let (authority, path) = match rest.split_once('/') {
        Some((auth, path)) => (auth, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port_str)) if !host.contains(':') => {
            let port = port_str.parse().ok()?;
            (host.to_string(), port)
        }
        _ => (authority.to_string(), default_port),
    };
    if ipv4::parse_ipv4(&host).is_none() && !is_valid_host(&host) {
        return None;
    }
    Some(HttpUrl {
        scheme,
        host,
        port,
        path,
    })
}

fn is_valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

pub fn fetch(url: &HttpUrl) -> Result<Vec<u8>> {
    match url.scheme {
        HttpScheme::Http => http_get(&url.host, url.port, &url.path),
        #[cfg(feature = "wget-tls")]
        HttpScheme::Https => https_get(&url.host, url.port, &url.path),
    }
}

pub fn http_get(host: &str, port: u16, path: &str) -> Result<Vec<u8>> {
    let addr = ipv4::parse_ipv4(host).ok_or(Error::INVAL)?;
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(sys::last_errno());
    }

    let mut sockaddr = ipv4::ipv4_to_sockaddr_in(addr);
    sockaddr.sin_port = port.to_be();
    set_socket_timeouts(fd)?;
    if unsafe {
        libc::connect(
            fd,
            &sockaddr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        let err = sys::last_errno();
        close_fd(fd);
        return Err(err);
    }

    let request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    if let Err(e) = write_all(fd, request.as_bytes()) {
        close_fd(fd);
        return Err(e);
    }

    let buf = read_response(fd)?;
    close_fd(fd);
    response_body(&buf)
}

#[cfg(feature = "wget-tls")]
pub fn https_get(host: &str, port: u16, path: &str) -> Result<Vec<u8>> {
    let addr = ipv4::parse_ipv4(host).ok_or(Error::INVAL)?;
    let request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    let buf = crate::net::tls::tls_http_request(
        addr,
        port,
        host,
        request.as_bytes(),
        MAX_HEADER_BYTES + MAX_BODY_BYTES,
    )?;
    response_body(&buf)
}

fn read_response(fd: RawFd) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match read(unsafe { BorrowedFd::borrow_raw(fd) }, &mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(Error::INTR) => {}
            Err(e) => return Err(e),
        }
        if buf.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
            return Err(Error::IO);
        }
    }
    Ok(buf)
}

fn response_body(buf: &[u8]) -> Result<Vec<u8>> {
    let header_end = find_header_end(buf).ok_or(Error::IO)?;
    Ok(buf[skip_header_terminator(header_end)..].to_vec())
}

fn set_socket_timeouts(fd: RawFd) -> Result<()> {
    let timeout = libc::timeval {
        tv_sec: 5,
        tv_usec: 0,
    };
    let len = std::mem::size_of::<libc::timeval>() as libc::socklen_t;
    for opt in [libc::SO_RCVTIMEO, libc::SO_SNDTIMEO] {
        if unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                opt,
                &timeout as *const _ as *const libc::c_void,
                len,
            )
        } < 0
        {
            return Err(sys::last_errno());
        }
    }
    Ok(())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| buf.windows(2).position(|w| w == b"\n\n").map(|i| i + 2))
}

fn skip_header_terminator(end: usize) -> usize {
    end
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

fn close_fd(fd: RawFd) {
    unsafe {
        io::close(fd);
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
        for word in text.split_whitespace().take(32) {
            let _ = parse_url(word);
            if !word.starts_with("http://") && !word.starts_with("https://") {
                let _ = parse_url(&format!("http://{word}"));
                #[cfg(feature = "wget-tls")]
                let _ = parse_url(&format!("https://{word}"));
            }
        }
    }

    let mut buf = data.to_vec();
    if find_header_end(&buf).is_none() && buf.len() + 4 <= MAX_HEADER_BYTES {
        buf.extend_from_slice(b"\r\n\r\n");
    }
    if buf.len() <= MAX_HEADER_BYTES {
        if let Some(end) = find_header_end(&buf) {
            let _ = skip_header_terminator(end);
            let _ = &buf[skip_header_terminator(end)..];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_url() {
        let url = parse_http_url("http://127.0.0.1:8080/cgi-bin/hello").unwrap();
        assert_eq!(url.scheme, HttpScheme::Http);
        assert_eq!(url.host, "127.0.0.1");
        assert_eq!(url.port, 8080);
        assert_eq!(url.path, "/cgi-bin/hello");
    }

    #[test]
    fn parses_default_port() {
        let url = parse_http_url("http://10.0.2.2/").unwrap();
        assert_eq!(url.port, 80);
        assert_eq!(url.path, "/");
    }

    #[cfg(feature = "wget-tls")]
    #[test]
    fn parses_https_url() {
        let url = parse_https_url("https://127.0.0.1/path").unwrap();
        assert_eq!(url.scheme, HttpScheme::Https);
        assert_eq!(url.port, 443);
        assert_eq!(url.path, "/path");
    }
}
