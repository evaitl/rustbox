#[cfg(target_os = "linux")]
pub mod dhcp;
#[cfg(all(feature = "applet-dnscached", target_os = "linux"))]
pub mod dns_cache;
#[cfg(all(feature = "applet-dig", target_os = "linux"))]
pub mod dns_query;
#[cfg(all(feature = "applet-dnscached", target_os = "linux"))]
pub mod dnscached;
#[cfg(all(feature = "applet-dnscached", target_os = "linux"))]
pub mod doh;
#[cfg(target_os = "linux")]
pub mod http;
#[cfg(target_os = "linux")]
pub mod http_client;
#[cfg(target_os = "linux")]
pub mod iface;
#[cfg(target_os = "linux")]
pub mod ipv4;
#[cfg(target_os = "linux")]
pub mod netcat;
#[cfg(target_os = "linux")]
pub mod ntp;
#[cfg(target_os = "linux")]
pub mod ping;
#[cfg(target_os = "linux")]
pub mod route;
#[cfg(all(feature = "applet-sshd", target_os = "linux"))]
pub mod sshd;
#[cfg(target_os = "linux")]
pub mod syslog;
#[cfg(feature = "tls")]
pub mod tls;
