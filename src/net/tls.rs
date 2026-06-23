//! Shared blocking TLS client (rustls + webpki roots).

use crate::net::ipv4;
use crate::sys::{Error, Result};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_secs(10);

static TLS_CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();

pub fn client_config() -> Arc<ClientConfig> {
    TLS_CONFIG
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            )
        })
        .clone()
}

/// Connect to `connect_addr`:`port`, use `sni_host` for TLS SNI, send `request`, return raw HTTP bytes.
pub fn tls_http_request(
    connect_addr: u32,
    port: u16,
    sni_host: &str,
    request: &[u8],
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let ip = ipv4::format_ipv4(connect_addr);
    let mut tcp = TcpStream::connect((ip.as_str(), port)).map_err(|_| Error::IO)?;
    tcp.set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|_| Error::IO)?;
    tcp.set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| Error::IO)?;

    let server_name = ServerName::try_from(sni_host.to_string()).map_err(|_| Error::IO)?;
    let config = client_config();
    let mut conn = ClientConnection::new(config, server_name).map_err(|_| Error::IO)?;
    let mut tls = rustls::Stream::new(&mut conn, &mut tcp);

    tls.write_all(request).map_err(|_| Error::IO)?;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match tls.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > max_bytes {
                    return Err(Error::IO);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Error::IO),
        }
    }
    Ok(buf)
}
