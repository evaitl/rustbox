use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rustix::fd::AsFd;
use rustix::io::{self, write};
use rustix::stdio;
use std::io::{Read, Write};

pub fn compress_bytes(input: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).map_err(|_| io::Errno::IO)?;
    encoder.finish().map_err(|_| io::Errno::IO)
}

pub fn decompress_bytes(input: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(input);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|_| io::Errno::IO)?;
    Ok(out)
}

pub fn compress_reader_to_writer<R: Read, W: AsFd>(mut input: R, output: W) -> io::Result<()> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut buf = [0u8; 8192];
    loop {
        let n = input.read(&mut buf).map_err(|_| io::Errno::IO)?;
        if n == 0 {
            break;
        }
        encoder.write_all(&buf[..n]).map_err(|_| io::Errno::IO)?;
    }
    let compressed = encoder.finish().map_err(|_| io::Errno::IO)?;
    write(&output, &compressed)?;
    Ok(())
}

pub fn decompress_reader_to_writer<R: Read, W: AsFd>(mut input: R, output: W) -> io::Result<()> {
    let mut decoder = GzDecoder::new(&mut input);
    let mut buf = [0u8; 8192];
    loop {
        let n = decoder.read(&mut buf).map_err(|_| io::Errno::IO)?;
        if n == 0 {
            break;
        }
        write(&output, &buf[..n])?;
    }
    Ok(())
}

pub fn compress_file_to_path(src: &str, dst: &str) -> io::Result<()> {
    let fd = crate::sys::open_read(src)?;
    let data = crate::sys::read_to_end(fd)?;
    let out = compress_bytes(&data)?;
    crate::sys::write_file(dst, &out)
}

pub fn decompress_file_to_path(src: &str, dst: &str) -> io::Result<()> {
    let fd = crate::sys::open_read(src)?;
    let data = crate::sys::read_to_end(fd)?;
    let out = decompress_bytes(&data)?;
    crate::sys::write_file(dst, &out)
}

pub fn compress_file_to_stdout(src: &str) -> io::Result<()> {
    let fd = crate::sys::open_read(src)?;
    let data = crate::sys::read_to_end(fd)?;
    let out = compress_bytes(&data)?;
    write(stdio::stdout(), &out)?;
    Ok(())
}

pub fn decompress_file_to_stdout(src: &str) -> io::Result<()> {
    let fd = crate::sys::open_read(src)?;
    let data = crate::sys::read_to_end(fd)?;
    let out = decompress_bytes(&data)?;
    write(stdio::stdout(), &out)?;
    Ok(())
}

pub fn decompress_stdin_to_stdout() -> io::Result<()> {
    let mut input = std::io::stdin().lock();
    decompress_reader_to_writer(&mut input, stdio::stdout())
}

pub fn compress_stdin_to_stdout() -> io::Result<()> {
    let mut input = std::io::stdin().lock();
    compress_reader_to_writer(&mut input, stdio::stdout())
}

#[cfg(feature = "fuzzing")]
pub fn fuzz_input(data: &[u8]) {
    const MAX_IN: usize = 64 * 1024;
    const MAX_OUT: usize = 256 * 1024;
    let data = if data.len() > MAX_IN {
        &data[..MAX_IN]
    } else {
        data
    };
    let mut decoder = GzDecoder::new(data);
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let Ok(n) = decoder.read(&mut buf) else {
            break;
        };
        if n == 0 {
            break;
        }
        if out.len() + n > MAX_OUT {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    if data.len() <= 16 * 1024 {
        let _ = compress_bytes(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let data = b"hello rustbox gzip\n".repeat(100);
        let compressed = compress_bytes(&data).unwrap();
        let restored = decompress_bytes(&compressed).unwrap();
        assert_eq!(restored, data);
    }
}
