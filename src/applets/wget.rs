use crate::net::http_client::{self, parse_url};
use crate::sys;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut quiet = false;
    let mut output_path: Option<&str> = None;
    let mut url: Option<&str> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-q" => quiet = true,
            "-O" => {
                i += 1;
                if i >= args.len() {
                    usage("wget", "option requires an argument -- 'O'");
                    return 1;
                }
                output_path = Some(args[i]);
            }
            "-h" | "--help" => {
                #[cfg(feature = "wget-tls")]
                usage(
                    "wget",
                    "usage: wget [-q] [-O FILE|-] URL  (http:// or https://, IPv4 hosts)",
                );
                #[cfg(not(feature = "wget-tls"))]
                usage(
                    "wget",
                    "usage: wget [-q] [-O FILE|-] URL  (http://, IPv4 hosts)",
                );
                return 0;
            }
            s if s.starts_with('-') => {
                usage("wget", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => url = Some(s),
        }
        i += 1;
    }

    let url = match url {
        Some(url) => url,
        None => {
            usage("wget", "missing URL");
            return 1;
        }
    };

    #[cfg(not(feature = "wget-tls"))]
    if url.starts_with("https://") {
        eprintln("wget: https:// URLs require building with --features wget-tls");
        return 1;
    }

    let parsed = match parse_url(url) {
        Some(parsed) => parsed,
        None => {
            eprintln(format!("wget: {url}: unsupported or invalid URL"));
            return 1;
        }
    };

    let body = match http_client::fetch(&parsed) {
        Ok(body) => body,
        Err(e) => {
            if !quiet {
                eprintln(format!("wget: {url}: {e}"));
            }
            return 1;
        }
    };

    let target = output_path.unwrap_or("-");
    if target == "-" {
        return match write_all_stdout(&body) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("wget: write error: {e}"));
                1
            }
        };
    }

    match write_body_to_file(target, &body) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("wget: {target}: {e}"));
            1
        }
    }
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_parse_args(input: &str) {
    let args: Vec<String> = input
        .split_whitespace()
        .take(64)
        .map(String::from)
        .collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let mut url: Option<&str> = None;
    let mut i = 0;
    while i < refs.len() {
        match refs[i] {
            "-q" => {}
            "-O" => {
                i += 1;
            }
            "-h" | "--help" => return,
            s if s.starts_with('-') => return,
            s => url = Some(s),
        }
        i += 1;
    }
    if let Some(url) = url {
        let _ = parse_url(url);
    }
}

fn write_all_stdout(body: &[u8]) -> sys::Result<()> {
    let mut off = 0;
    while off < body.len() {
        off += rustix::io::write(rustix::stdio::stdout(), &body[off..])?;
    }
    Ok(())
}

fn write_body_to_file(path: &str, body: &[u8]) -> sys::Result<()> {
    let fd = sys::open_create(path)?;
    rustix::io::write(&fd, body)?;
    Ok(())
}
