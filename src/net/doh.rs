//! DNS-over-HTTPS client (blocking TLS, pinned upstream IPv4 addresses).

use crate::net::tls;
use crate::sys::{Error, Result};
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Upstream DoH parameters (addresses are literal IPv4; TLS uses `host` for SNI/Host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upstream {
    pub addrs: Vec<u32>,
    pub host: String,
    pub path: String,
}

static UPSTREAM_ROTATE: AtomicUsize = AtomicUsize::new(0);

/// POST `query` to the next configured upstream (round-robin with failover).
pub fn doh_query(upstream: &Upstream, query: &[u8]) -> Result<Vec<u8>> {
    if upstream.addrs.is_empty() {
        return Err(Error::INVAL);
    }
    let n = upstream.addrs.len();
    let start = UPSTREAM_ROTATE.fetch_add(1, Ordering::Relaxed) % n;
    let mut last_err = Error::IO;
    for i in 0..n {
        let addr = upstream.addrs[(start + i) % n];
        match doh_query_one(addr, upstream, query) {
            Ok(body) => return Ok(body),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

fn doh_query_one(addr: u32, upstream: &Upstream, query: &[u8]) -> Result<Vec<u8>> {
    let path = if upstream.path.is_empty() {
        "/dns-query"
    } else {
        upstream.path.as_str()
    };
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/dns-message\r\nAccept: application/dns-message\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        upstream.host,
        query.len()
    );
    let mut payload = request.into_bytes();
    payload.extend_from_slice(query);

    let response = tls::tls_http_request(addr, 443, &upstream.host, &payload, MAX_RESPONSE_BYTES)?;
    extract_doh_body(&response)
}

fn extract_doh_body(response: &[u8]) -> Result<Vec<u8>> {
    let header_end = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .ok_or(Error::IO)?;
    let header = &response[..header_end];
    if !header.starts_with(b"HTTP/1.") {
        return Err(Error::IO);
    }
    let status_line = header
        .split(|&b| b == b'\r' || b == b'\n')
        .next()
        .ok_or(Error::IO)?;
    let status = std::str::from_utf8(status_line).map_err(|_| Error::IO)?;
    if !status.contains(" 200 ") {
        return Err(Error::IO);
    }
    Ok(response[header_end..].to_vec())
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_input(data: &[u8]) {
    let mut buf = data.to_vec();
    if buf.len() + 4 <= MAX_RESPONSE_BYTES && !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        buf.extend_from_slice(b"\r\n\r\n");
    }
    let _ = extract_doh_body(&buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_doh_body_from_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: 2\r\n\r\n\xab\xcd";
        assert_eq!(extract_doh_body(raw).unwrap(), b"\xab\xcd");
    }
}
