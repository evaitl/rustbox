mod support;

use std::fs;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use support::Rustbox;

#[test]
fn new_applets_listed() {
    let out = Rustbox::new().args(["--list"]).stdout();
    for name in ["nc", "dig", "logger", "ntpclient"] {
        assert!(
            out.lines().any(|line| line.trim() == name),
            "missing applet {name}"
        );
    }
}

#[test]
fn logger_help() {
    assert_eq!(Rustbox::new().applet("logger").arg("--help").status(), 0);
}

#[test]
fn dig_help_is_usage_error() {
    let status = Rustbox::new().applet("dig").status();
    assert_eq!(status, 1);
}

#[test]
fn nc_help() {
    assert_eq!(Rustbox::new().applet("nc").arg("--help").status(), 0);
}

#[test]
fn ntpclient_help() {
    assert_eq!(Rustbox::new().applet("ntpclient").arg("--help").status(), 0);
}

#[test]
fn logger_writes_to_unix_socket() {
    if !cfg!(target_os = "linux") {
        return;
    }

    let dir = tempfile_dir();
    let socket_path = dir.join("log");
    let log_path = dir.join("messages");

    let _listener = UnixDatagram::bind(&socket_path).expect("bind unix socket");
    _listener
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");

    let status = Rustbox::new()
        .applet("logger")
        .args(["-S", socket_path.to_str().unwrap(), "-t", "test", "hello"])
        .status();
    assert_eq!(status, 0);

    let mut buf = [0u8; 256];
    let n = _listener.recv(&mut buf).expect("recv log message");
    let msg = String::from_utf8_lossy(&buf[..n]);
    assert!(msg.contains("<13>"));
    assert!(msg.contains("test: hello"));

    let _ = log_path;
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn nc_tcp_connect_exits_zero() {
    if !cfg!(target_os = "linux") {
        return;
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();

    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        drop(stream);
    });

    let status = Rustbox::new()
        .applet("nc")
        .args(["-w", "2", "127.0.0.1", &port.to_string()])
        .status();
    assert_eq!(status, 0);
}

fn tempfile_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rustbox-logger-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("mkdir temp");
    dir
}
