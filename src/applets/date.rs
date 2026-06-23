use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    let mut utc = false;
    let mut format: Option<&str> = None;

    for arg in args {
        match *arg {
            "-u" => utc = true,
            s if s.starts_with('+') => format = Some(&s[1..]),
            s if s.starts_with('-') => {
                usage("date", &format!("invalid option -- '{s}'"));
                return 1;
            }
            _ => {
                usage("date", "extra operand");
                return 1;
            }
        }
    }

    let fmt = format.unwrap_or("%a %b %e %H:%M:%S %Z %Y");
    match format_time(fmt, utc) {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(msg) => {
            eprintln(format!("date: {msg}"));
            1
        }
    }
}

fn format_time(fmt: &str, utc: bool) -> Result<String, &'static str> {
    let c_fmt = std::ffi::CString::new(fmt).map_err(|_| "invalid format string")?;
    let mut buf = [0u8; 256];

    unsafe {
        let secs = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::MaybeUninit::<libc::tm>::zeroed();
        let tm_ptr = if utc {
            libc::gmtime_r(&secs, tm.as_mut_ptr())
        } else {
            libc::localtime_r(&secs, tm.as_mut_ptr())
        };
        if tm_ptr.is_null() {
            return Err("cannot read time");
        }
        let n = libc::strftime(
            buf.as_mut_ptr().cast(),
            buf.len(),
            c_fmt.as_ptr(),
            tm.as_mut_ptr(),
        );
        if n == 0 {
            return Err("invalid format string");
        }
        Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
    }
}
