use crate::sys::{self, Error};
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut from_file = false;
    let mut name: Option<&str> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-F" => {
                from_file = true;
                i += 1;
                if i >= args.len() {
                    usage("hostname", "option requires an argument -- 'F'");
                    return 1;
                }
                name = Some(args[i]);
            }
            "-f" => {
                i += 1;
                if i >= args.len() {
                    usage("hostname", "option requires an argument -- 'f'");
                    return 1;
                }
                name = Some(args[i]);
            }
            s if s.starts_with('-') => {
                usage("hostname", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => name = Some(s),
        }
        i += 1;
    }

    if let Some(n) = name {
        let host = if from_file {
            match sys::read_to_string(n) {
                Ok(s) => s.trim().to_string(),
                Err(e) => {
                    eprintln(format!("hostname: {e}"));
                    return 1;
                }
            }
        } else {
            n.to_string()
        };
        if host.is_empty() {
            return 1;
        }
        if host.len() > 253 {
            usage("hostname", "name too long");
            return 1;
        }
        return match set_hostname(&host) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("hostname: {e}"));
                1
            }
        };
    }

    match get_hostname() {
        Ok(h) => {
            println!("{h}");
            0
        }
        Err(e) => {
            eprintln(format!("hostname: {e}"));
            1
        }
    }
}

fn get_hostname() -> Result<String, Error> {
    let mut buf = [0u8; 256];
    if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } != 0 {
        return Err(sys::last_errno());
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

fn set_hostname(name: &str) -> Result<(), Error> {
    let cname = std::ffi::CString::new(name).map_err(|_| Error::INVAL)?;
    if unsafe { libc::sethostname(cname.as_ptr(), cname.as_bytes().len()) } != 0 {
        return Err(sys::last_errno());
    }
    Ok(())
}
