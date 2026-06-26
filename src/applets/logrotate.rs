use crate::compress::gzip;
use crate::sys;
use crate::{eprintln, usage};
use std::path::Path;

#[derive(Clone, Debug, Default)]
struct Stanza {
    path: String,
    daily: bool,
    rotate: u32,
    compress: bool,
    maxsize: Option<u64>,
    totalsize: Option<u64>,
    missingok: bool,
    notifempty: bool,
}

pub fn run(args: &[&str]) -> i32 {
    let mut force = false;
    let mut configs: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" | "--force" => force = true,
            "-h" | "--help" => {
                usage("logrotate", "usage: logrotate [-f] CONFIG...");
                return 0;
            }
            s if s.starts_with('-') => {
                usage("logrotate", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => configs.push(s),
        }
        i += 1;
    }

    if configs.is_empty() {
        configs.push("/etc/logrotate.conf");
    }

    let mut stanzas = Vec::new();
    for path in configs {
        match load_config(path) {
            Ok(mut parsed) => stanzas.append(&mut parsed),
            Err(e) => {
                eprintln(format!("logrotate: {path}: {e}"));
                return 1;
            }
        }
    }

    for stanza in &stanzas {
        if let Err(e) = rotate_one(stanza, force) {
            eprintln(format!("logrotate: {}: {e}", stanza.path));
            return 1;
        }
    }
    0
}

fn load_config(path: &str) -> Result<Vec<Stanza>, String> {
    let text = sys::read_to_string(path).map_err(|e| format!("{e}"))?;
    let mut stanzas = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(dir) = line.strip_prefix("include ") {
            let dir = dir.trim();
            if sys::is_directory(dir) {
                for entry in sys::read_dir(dir).map_err(|e| format!("cannot read '{dir}': {e}"))? {
                    if entry.file_type.is_file() {
                        let child = join_path(dir, &entry.name);
                        stanzas.extend(load_config(&child)?);
                    }
                }
            }
            continue;
        }
        if !line.ends_with('{') {
            return Err(format!("expected '{{' after '{line}'"));
        }
        let path = line.trim_end_matches('{').trim().to_string();
        let mut stanza = Stanza {
            path,
            rotate: 1,
            ..Stanza::default()
        };
        for body in lines.by_ref() {
            let body = body.split('#').next().unwrap_or(body).trim();
            if body.is_empty() {
                continue;
            }
            if body == "}" {
                break;
            }
            apply_directive(&mut stanza, body)?;
        }
        stanzas.push(stanza);
    }
    Ok(stanzas)
}

fn apply_directive(stanza: &mut Stanza, line: &str) -> Result<(), String> {
    let mut parts = line.split_whitespace();
    let key = parts.next().unwrap_or("").to_ascii_lowercase();
    match key.as_str() {
        "daily" => stanza.daily = true,
        "rotate" => {
            let n = parts
                .next()
                .ok_or_else(|| "rotate requires a count".to_string())?
                .parse::<u32>()
                .map_err(|_| "invalid rotate count".to_string())?;
            stanza.rotate = n;
        }
        "compress" => stanza.compress = true,
        "maxsize" | "size" => {
            let raw = parts
                .next()
                .ok_or_else(|| format!("{key} requires a value"))?;
            stanza.maxsize = Some(parse_size(raw)?);
        }
        "totalsize" => {
            let raw = parts
                .next()
                .ok_or_else(|| "totalsize requires a value".to_string())?;
            stanza.totalsize = Some(parse_size(raw)?);
        }
        "missingok" => stanza.missingok = true,
        "notifempty" => stanza.notifempty = true,
        _ => return Err(format!("unknown directive '{key}'")),
    }
    Ok(())
}

fn rotate_one(stanza: &Stanza, force: bool) -> Result<(), String> {
    let path = &stanza.path;
    if !sys::exists(path) {
        if stanza.missingok {
            return Ok(());
        }
        return Err("log file missing".into());
    }

    let size = sys::file_size(path).map_err(|e| format!("{e}"))?;
    if stanza.notifempty && size == 0 {
        return Ok(());
    }

    let should_rotate = force || stanza.daily || stanza.maxsize.is_some_and(|limit| size >= limit);
    if !should_rotate {
        return Ok(());
    }

    shift_rotations(path, stanza.rotate, stanza.compress)?;
    let first = format!("{path}.1");
    sys::rename_path(path, &first).map_err(|e| format!("cannot rotate '{path}': {e}"))?;

    if stanza.compress {
        let gz = format!("{first}.gz");
        gzip::compress_file_to_path(&first, &gz).map_err(|e| format!("cannot compress: {e}"))?;
        let _ = sys::remove_file(&first);
    }

    if let Some(limit) = stanza.totalsize {
        enforce_totalsize(path, limit)?;
    }

    let _ = sys::write_file(path, &[]);
    Ok(())
}

fn shift_rotations(path: &str, rotate: u32, compress: bool) -> Result<(), String> {
    if rotate == 0 {
        return Ok(());
    }
    let drop = rotated_name(path, rotate, compress);
    if sys::exists(&drop) {
        sys::remove_file(&drop).map_err(|e| format!("cannot remove '{drop}': {e}"))?;
    }
    for n in (1..rotate).rev() {
        let src = rotated_name(path, n, compress);
        let dst = rotated_name(path, n + 1, compress);
        if sys::exists(&src) {
            sys::rename_path(&src, &dst).map_err(|e| format!("cannot rename '{src}': {e}"))?;
        }
    }
    Ok(())
}

fn rotated_name(path: &str, n: u32, compress: bool) -> String {
    if compress {
        format!("{path}.{n}.gz")
    } else {
        format!("{path}.{n}")
    }
}

fn enforce_totalsize(path: &str, limit: u64) -> Result<(), String> {
    let mut files = related_files(path);
    loop {
        let total: u64 = files.iter().filter_map(|f| sys::file_size(f).ok()).sum();
        if total <= limit {
            return Ok(());
        }
        let Some(oldest) = files
            .iter()
            .filter(|f| *f != path)
            .max_by_key(|f| rotation_index(path, f))
            .cloned()
        else {
            return Ok(());
        };
        sys::remove_file(&oldest).map_err(|e| format!("cannot remove '{oldest}': {e}"))?;
        files.retain(|f| f != &oldest);
    }
}

fn related_files(path: &str) -> Vec<String> {
    let parent = Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");
    let base = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    let mut files = vec![path.to_string()];
    if let Ok(entries) = sys::read_dir(parent) {
        for entry in entries {
            if entry.name == base {
                continue;
            }
            if let Some(rest) = entry.name.strip_prefix(&format!("{base}.")) {
                if rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    files.push(join_path(parent, &entry.name));
                }
            }
        }
    }
    files
}

fn rotation_index(base: &str, candidate: &str) -> u32 {
    let base_name = Path::new(base)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(base);
    let file = Path::new(candidate)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(candidate);
    let rest = file.strip_prefix(&format!("{base_name}.")).unwrap_or("");
    let num = rest.strip_suffix(".gz").unwrap_or(rest);
    num.parse().unwrap_or(0)
}

fn parse_size(raw: &str) -> Result<u64, String> {
    let raw = raw.trim();
    let (num, mult) = if let Some(n) = raw.strip_suffix(['k', 'K']) {
        (n, 1024u64)
    } else if let Some(n) = raw.strip_suffix(['m', 'M']) {
        (n, 1024 * 1024)
    } else {
        (raw, 1)
    };
    let value: u64 = num.parse().map_err(|_| format!("invalid size '{raw}'"))?;
    Ok(value.saturating_mul(mult))
}

fn join_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config_stanza() {
        let text = r#"/var/log/messages {
    daily
    rotate 3
    compress
    totalsize 1536k
    missingok
}"#;
        let mut lines = text.lines().peekable();
        let line = lines.next().unwrap().trim();
        let path = line.trim_end_matches('{').trim().to_string();
        let mut stanza = Stanza {
            path,
            rotate: 1,
            ..Stanza::default()
        };
        for body in lines {
            let body = body.trim();
            if body == "}" {
                break;
            }
            apply_directive(&mut stanza, body).unwrap();
        }
        assert!(stanza.daily);
        assert_eq!(stanza.rotate, 3);
        assert!(stanza.compress);
        assert_eq!(stanza.totalsize, Some(1536 * 1024));
    }

    #[test]
    fn parse_size_suffixes() {
        assert_eq!(parse_size("500k").unwrap(), 512_000);
        assert_eq!(parse_size("2m").unwrap(), 2 * 1024 * 1024);
    }
}
