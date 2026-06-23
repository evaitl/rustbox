use crate::sys;
use crate::{eprintln, usage};
use rustix::fs::{seek, SeekFrom};
use rustix::io::{read, write};
use rustix::stdio;

pub fn run(args: &[&str]) -> i32 {
    let mut opts = DdOpts::default();

    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            if let Err(msg) = opts.set(key, value) {
                usage("dd", msg);
                return 1;
            }
        } else {
            usage("dd", &format!("invalid operand '{arg}'"));
            return 1;
        }
    }

    if let Err(msg) = opts.validate() {
        usage("dd", msg);
        return 1;
    }

    match run_dd(&opts) {
        Ok(()) => 0,
        Err(e) => {
            eprintln(format!("dd: {e}"));
            1
        }
    }
}

#[derive(Default)]
struct DdOpts {
    if_path: Option<String>,
    of_path: Option<String>,
    bs: usize,
    count: Option<u64>,
    skip: u64,
    seek: u64,
}

impl DdOpts {
    fn set(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        match key {
            "if" => self.if_path = Some(value.to_string()),
            "of" => self.of_path = Some(value.to_string()),
            "bs" => self.bs = parse_usize(value).ok_or("invalid bs value")?,
            "count" => self.count = Some(parse_u64(value).ok_or("invalid count value")?),
            "skip" => self.skip = parse_u64(value).ok_or("invalid skip value")?,
            "seek" => self.seek = parse_u64(value).ok_or("invalid seek value")?,
            _ => return Err("unknown operand"),
        }
        Ok(())
    }

    fn validate(&mut self) -> Result<(), &'static str> {
        if self.bs == 0 {
            self.bs = 512;
        }
        Ok(())
    }
}

fn run_dd(opts: &DdOpts) -> Result<(), String> {
    let mut input = open_input(opts.if_path.as_deref())?;
    let mut output = open_output(opts.of_path.as_deref())?;

    if opts.skip > 0 {
        discard_blocks(&mut input, opts.bs, opts.skip)?;
    }

    if opts.seek > 0 {
        if let Output::File(fd) = &output {
            seek(
                fd,
                SeekFrom::Start(opts.seek.saturating_mul(opts.bs as u64)),
            )
            .map_err(|e| e.to_string())?;
        }
    }

    let blocks = opts.count.unwrap_or(u64::MAX);
    let mut buf = vec![0u8; opts.bs];
    for _ in 0..blocks {
        let n = read_block(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }
        write_block(&mut output, &buf[..n])?;
        if n < opts.bs {
            break;
        }
    }
    Ok(())
}

enum Input {
    Stdin,
    File(rustix::fd::OwnedFd),
}

enum Output {
    Stdout,
    File(rustix::fd::OwnedFd),
}

fn open_input(path: Option<&str>) -> Result<Input, String> {
    match path {
        None => Ok(Input::Stdin),
        Some(p) => sys::open_read(p)
            .map(Input::File)
            .map_err(|e| format!("cannot open '{p}': {e}")),
    }
}

fn open_output(path: Option<&str>) -> Result<Output, String> {
    match path {
        None => Ok(Output::Stdout),
        Some(p) => sys::open_create(p)
            .map(Output::File)
            .map_err(|e| format!("cannot open '{p}': {e}")),
    }
}

fn discard_blocks(input: &mut Input, bs: usize, blocks: u64) -> Result<(), String> {
    let mut buf = vec![0u8; bs];
    for _ in 0..blocks {
        let n = read_block(input, &mut buf)?;
        if n == 0 {
            break;
        }
    }
    Ok(())
}

fn read_block(input: &mut Input, buf: &mut [u8]) -> Result<usize, String> {
    match input {
        Input::Stdin => read(stdio::stdin(), buf).map_err(|e| e.to_string()),
        Input::File(fd) => read(fd, buf).map_err(|e| e.to_string()),
    }
}

fn write_block(output: &mut Output, buf: &[u8]) -> Result<(), String> {
    match output {
        Output::Stdout => {
            let mut off = 0;
            while off < buf.len() {
                off += write(stdio::stdout(), &buf[off..]).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        Output::File(fd) => {
            let mut off = 0;
            while off < buf.len() {
                off += write(&fd, &buf[off..]).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }
}

fn parse_usize(s: &str) -> Option<usize> {
    s.parse::<usize>().ok()
}

fn parse_u64(s: &str) -> Option<u64> {
    s.parse::<u64>().ok()
}
