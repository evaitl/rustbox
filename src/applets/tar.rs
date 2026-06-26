use crate::compress::gzip;
use crate::sys;
use crate::{eprintln, usage};
use rustix::io::write;
use rustix::stdio;

const BLOCK: usize = 512;

pub fn run(args: &[&str]) -> i32 {
    let mut create = false;
    let mut extract = false;
    let mut list = false;
    let mut gzip_filter = false;
    let mut archive: Option<String> = None;
    let mut members: Vec<&str> = Vec::new();
    let mut verbose = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == "-h" || arg == "--help" {
            usage("tar", "usage: tar [-czxvt] [-f ARCHIVE] [FILE]...");
            return 0;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            let mut rest = &arg[1..];
            while !rest.is_empty() {
                let ch = match rest.chars().next() {
                    Some(c) => c,
                    None => break,
                };
                let ch_len = ch.len_utf8();
                rest = &rest[ch_len..];
                match ch {
                    'c' => create = true,
                    'x' => extract = true,
                    't' => list = true,
                    'z' => gzip_filter = true,
                    'v' => verbose = true,
                    'f' => {
                        let path = if rest.is_empty() {
                            i += 1;
                            if i >= args.len() {
                                usage("tar", "option requires an argument -- 'f'");
                                return 1;
                            }
                            args[i].to_string()
                        } else {
                            let path = rest.to_string();
                            rest = "";
                            path
                        };
                        archive = Some(path);
                    }
                    _ => {
                        usage("tar", &format!("invalid option -- '{ch}'"));
                        return 1;
                    }
                }
            }
            i += 1;
            continue;
        }
        members.push(arg);
        i += 1;
    }

    if archive.is_none() {
        usage("tar", "option requires an argument -- 'f'");
        return 1;
    }
    let archive = archive.unwrap();

    if create {
        if members.is_empty() {
            usage("tar", "missing file operand");
            return 1;
        }
        return match create_archive(&archive, &members, gzip_filter, verbose) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("tar: {e}"));
                1
            }
        };
    }
    if extract {
        return match extract_archive(&archive, gzip_filter, verbose) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("tar: {e}"));
                1
            }
        };
    }
    if list {
        return match list_archive(&archive, gzip_filter) {
            Ok(()) => 0,
            Err(e) => {
                eprintln(format!("tar: {e}"));
                1
            }
        };
    }

    usage("tar", "you must specify one of -c, -x, or -t");
    1
}

fn create_archive(
    archive: &str,
    members: &[&str],
    gzip_filter: bool,
    verbose: bool,
) -> Result<(), String> {
    let mut data = Vec::new();
    for member in members {
        if sys::is_directory(member) {
            add_dir(&mut data, member, member, verbose)?;
        } else {
            add_file(&mut data, member, member, verbose)?;
        }
    }
    data.extend(std::iter::repeat_n(0u8, BLOCK));
    write_archive(archive, &data, gzip_filter)
}

fn add_dir(data: &mut Vec<u8>, root: &str, path: &str, verbose: bool) -> Result<(), String> {
    if verbose {
        eprintln(format!("tar: {path}"));
    }
    write_header(data, path, 0, b'5', 0o755)?;
    for entry in sys::read_dir(path).map_err(|e| format!("cannot read '{path}': {e}"))? {
        if entry.name == "." || entry.name == ".." {
            continue;
        }
        let child = join_path(path, &entry.name);
        let arc_name = join_path(root, &entry.name);
        if entry.file_type.is_dir() {
            add_dir(data, &arc_name, &child, verbose)?;
        } else if entry.file_type.is_file() {
            add_file(data, &child, &arc_name, verbose)?;
        }
    }
    Ok(())
}

fn add_file(data: &mut Vec<u8>, path: &str, arc_name: &str, verbose: bool) -> Result<(), String> {
    if verbose {
        eprintln(format!("tar: {arc_name}"));
    }
    let fd = sys::open_read(path).map_err(|e| format!("cannot open '{path}': {e}"))?;
    let body = sys::read_to_end(fd).map_err(|e| format!("cannot read '{path}': {e}"))?;
    let st = sys::stat(path).map_err(|e| format!("cannot stat '{path}': {e}"))?;
    let mode = (st.st_mode & 0o7777) as u32;
    write_header(data, arc_name, body.len() as u64, b'0', mode)?;
    data.extend_from_slice(&body);
    pad_block(data);
    Ok(())
}

fn extract_archive(archive: &str, gzip_filter: bool, verbose: bool) -> Result<(), String> {
    let data = read_archive(archive, gzip_filter)?;
    let mut offset = 0usize;
    while offset + BLOCK <= data.len() {
        let header = &data[offset..offset + BLOCK];
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let (name, size, kind) = parse_header(header)?;
        offset += BLOCK;
        let end = offset + size;
        if end > data.len() {
            return Err("truncated archive".into());
        }
        let body = &data[offset..end];
        offset = end;
        if size % BLOCK != 0 {
            offset += BLOCK - (size % BLOCK);
        }
        match kind {
            b'0' | b'\0' => {
                if verbose {
                    eprintln(format!("tar: {name}"));
                }
                if let Some(parent) = parent_of(&name) {
                    let _ = sys::mkdir_all(parent);
                }
                sys::write_file(&name, body).map_err(|e| format!("cannot write '{name}': {e}"))?;
            }
            b'5' => {
                if verbose {
                    eprintln(format!("tar: {name}"));
                }
                let _ = sys::mkdir_all(name.trim_end_matches('/'));
            }
            _ => {}
        }
    }
    Ok(())
}

fn list_archive(archive: &str, gzip_filter: bool) -> Result<(), String> {
    let data = read_archive(archive, gzip_filter)?;
    let mut offset = 0usize;
    while offset + BLOCK <= data.len() {
        let header = &data[offset..offset + BLOCK];
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let (name, size, _kind) = parse_header(header)?;
        write(stdio::stdout(), name.as_bytes()).map_err(|e| format!("{e}"))?;
        write(stdio::stdout(), b"\n").map_err(|e| format!("{e}"))?;
        offset += BLOCK + size;
        if size % BLOCK != 0 {
            offset += BLOCK - (size % BLOCK);
        }
    }
    Ok(())
}

fn read_archive(path: &str, gzip_filter: bool) -> Result<Vec<u8>, String> {
    let fd = sys::open_read(path).map_err(|e| format!("cannot open '{path}': {e}"))?;
    let raw = sys::read_to_end(fd).map_err(|e| format!("cannot read '{path}': {e}"))?;
    if gzip_filter {
        gzip::decompress_bytes(&raw).map_err(|e| format!("cannot decompress '{path}': {e}"))
    } else {
        Ok(raw)
    }
}

fn write_archive(path: &str, data: &[u8], gzip_filter: bool) -> Result<(), String> {
    let out = if gzip_filter {
        gzip::compress_bytes(data).map_err(|e| format!("cannot compress '{path}': {e}"))?
    } else {
        data.to_vec()
    };
    sys::write_file(path, &out).map_err(|e| format!("cannot write '{path}': {e}"))
}

fn write_header(
    data: &mut Vec<u8>,
    name: &str,
    size: u64,
    kind: u8,
    mode: u32,
) -> Result<(), String> {
    let mut block = [0u8; BLOCK];
    let (prefix, base) = split_name(name);
    if base.len() > 100 {
        return Err(format!("path name too long: {name}"));
    }
    block[..base.len()].copy_from_slice(base.as_bytes());
    if !prefix.is_empty() {
        let p = prefix.as_bytes();
        if p.len() > 155 {
            return Err(format!("path prefix too long: {name}"));
        }
        block[345..345 + p.len()].copy_from_slice(p);
    }
    write_octal(&mut block[100..108], u64::from(mode & 0o7777), 7);
    write_octal(&mut block[108..116], 0, 7);
    write_octal(&mut block[116..124], 0, 7);
    write_octal(&mut block[124..136], size, 11);
    write_octal(&mut block[136..148], 0, 11);
    block[156] = kind;
    block[257..262].copy_from_slice(b"ustar");
    block[262..264].copy_from_slice(b"00");
    write_checksum(&mut block);
    data.extend_from_slice(&block);
    Ok(())
}

fn parse_header(block: &[u8]) -> Result<(String, usize, u8), String> {
    let base = field_str(&block[..100]);
    let prefix = field_str(&block[345..500]);
    let name = if prefix.is_empty() {
        base
    } else {
        format!("{prefix}/{base}")
    };
    let size = read_octal(&block[124..136])?;
    let kind = block[156];
    Ok((name, size, kind))
}

fn write_checksum(block: &mut [u8; BLOCK]) {
    for b in &mut block[148..156] {
        *b = b' ';
    }
    let sum: u64 = block.iter().map(|&b| b as u64).sum();
    write_octal(&mut block[148..156], sum, 6);
    block[155] = 0;
}

fn write_octal(dst: &mut [u8], value: u64, width: usize) {
    let mut buf = format!("{value:o}");
    if buf.len() > width {
        buf = "0".repeat(width);
    }
    let pad = width.saturating_sub(buf.len());
    for b in &mut dst[..pad] {
        *b = b'0';
    }
    dst[pad..pad + buf.len()].copy_from_slice(buf.as_bytes());
    dst[pad + buf.len()] = 0;
}

fn read_octal(field: &[u8]) -> Result<usize, String> {
    let s = field_str(field);
    if s.is_empty() {
        return Ok(0);
    }
    usize::from_str_radix(&s, 8).map_err(|_| "invalid tar header".into())
}

fn field_str(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).trim().to_string()
}

fn pad_block(data: &mut Vec<u8>) {
    let rem = data.len() % BLOCK;
    if rem != 0 {
        data.extend(std::iter::repeat_n(0u8, BLOCK - rem));
    }
}

fn split_name(path: &str) -> (String, String) {
    let path = path.trim_start_matches("./");
    if let Some(idx) = path.rfind('/') {
        let (prefix, base) = path.split_at(idx);
        (prefix.to_string(), base.trim_start_matches('/').to_string())
    } else {
        (String::new(), path.to_string())
    }
}

fn join_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn parent_of(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    path.rfind('/')
        .map(|idx| &path[..idx])
        .filter(|p| !p.is_empty())
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_parse_args(input: &str) {
    let args: Vec<String> = input
        .split_whitespace()
        .take(32)
        .map(String::from)
        .collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let _ = run(&refs);
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_input(data: &[u8]) {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    const MAX: usize = 128 * 1024;
    let data = if data.len() > MAX { &data[..MAX] } else { data };

    let dir = std::env::temp_dir().join(format!("rustbox-tar-fuzz-{}", std::process::id()));
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let archive = dir.join("archive");
    let _ = std::fs::write(&archive, data);
    let Some(archive_s) = archive.to_str() else {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = run(&["-tf", archive_s]);
        let _ = run(&["-tzf", archive_s]);
    }));
    let _ = std::fs::remove_dir_all(&dir);
}
