//! Bcrypt password hashes stored in field 2 of `/etc/passwd` lines.

use crate::passwd_lookup;
use crate::sys::{self, Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_AUTH_PASSWD_PATH: &str = passwd_lookup::DEFAULT_PASSWD;
pub const BCRYPT_COST: u32 = 12;

const PASSWD_FIELDS: usize = 7;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct AuthPasswdTable {
    entries: HashMap<String, String>,
}

impl AuthPasswdTable {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, user: &str) -> Option<&str> {
        self.entries.get(user).map(String::as_str)
    }

    pub fn check(&self, user: &str, password: &str) -> bool {
        self.entries
            .get(user)
            .is_some_and(|stored| verify_password_hash(stored, password))
    }
}

pub fn verify_password_hash(stored: &str, password: &str) -> bool {
    if stored.starts_with("$2a$") || stored.starts_with("$2b$") || stored.starts_with("$2y$") {
        bcrypt::verify(password, stored).unwrap_or(false)
    } else {
        false
    }
}

pub fn hash_password(password: &str) -> Result<String> {
    bcrypt::hash(password, BCRYPT_COST).map_err(|_| Error::IO)
}

pub fn load_auth_passwd(path: &str) -> AuthPasswdTable {
    if !Path::new(path).exists() {
        return AuthPasswdTable::default();
    }
    match sys::read_to_string(path) {
        Ok(text) => parse_auth_passwd_text(&text),
        Err(_) => AuthPasswdTable::default(),
    }
}

pub fn parse_auth_passwd_text(text: &str) -> AuthPasswdTable {
    let mut entries = HashMap::new();
    for line in text.lines() {
        let Some((fields, _comment)) = split_passwd_fields(line) else {
            continue;
        };
        let user = fields[0].trim();
        let pass = fields[1].trim();
        if user.is_empty() || !is_bcrypt_hash(pass) {
            continue;
        }
        entries.insert(user.to_string(), pass.to_string());
    }
    AuthPasswdTable { entries }
}

pub fn update_auth_passwd(path: &str, user: &str, new_hash: &str) -> Result<()> {
    let path = Path::new(path);
    let original = if path.exists() {
        sys::read_to_string(path.to_str().ok_or(Error::IO)?)?
    } else {
        String::new()
    };

    if !user_is_present(&original, user) && original.trim().is_empty() {
        return Err(Error::NOENT);
    }

    let updated = rewrite_auth_passwd_text(&original, user, new_hash);
    write_auth_passwd_atomic(path, &updated)
}

fn is_bcrypt_hash(value: &str) -> bool {
    value.starts_with("$2a$") || value.starts_with("$2b$") || value.starts_with("$2y$")
}

fn user_is_present(text: &str, user: &str) -> bool {
    passwd_lookup::parse_passwd_text(text)
        .iter()
        .any(|entry| entry.name == user)
}

fn split_trailing_comment(line: &str) -> (&str, Option<&str>) {
    match line.find(" #") {
        Some(idx) => (&line[..idx], Some(&line[idx..])),
        None => (line, None),
    }
}

fn split_passwd_fields(line: &str) -> Option<(Vec<&str>, Option<&str>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let (body, comment) = split_trailing_comment(line.trim_end());
    let mut fields = Vec::with_capacity(PASSWD_FIELDS);
    let mut rest = body;
    for _ in 0..PASSWD_FIELDS - 1 {
        let (field, remainder) = rest.split_once(':')?;
        fields.push(field);
        rest = remainder;
    }
    fields.push(rest);
    if fields.len() < PASSWD_FIELDS {
        return None;
    }
    Some((fields, comment))
}

fn format_passwd_line(fields: &[&str], comment: Option<&str>) -> String {
    let mut out = fields.join(":");
    if let Some(comment) = comment {
        out.push_str(comment);
    }
    out
}

fn rewrite_auth_passwd_text(text: &str, user: &str, new_hash: &str) -> String {
    let mut out = String::new();
    let mut replaced = false;

    for line in text.lines() {
        if let Some((mut fields, comment)) = split_passwd_fields(line) {
            if fields[0].trim() == user {
                fields[1] = new_hash;
                out.push_str(&format_passwd_line(&fields, comment));
                out.push('\n');
                replaced = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    if !replaced {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(user);
        out.push(':');
        out.push_str(new_hash);
        out.push_str(":0:0::/root:/bin/rash\n");
    }

    out
}

fn write_auth_passwd_atomic(path: &Path, text: &str) -> Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let mut tmp =
        PathBuf::from(parent.map_or_else(|| "/tmp".to_string(), |p| p.display().to_string()));
    tmp.push(format!(
        ".{}.passwd.new",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("auth")
    ));

    sys::open_create(&tmp.to_string_lossy())?;
    std::fs::write(&tmp, text).map_err(|_| Error::IO)?;
    rustix::fs::rename(&tmp, path).map_err(|_| Error::IO)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_line(hash: &str) -> String {
        format!("rustbox:{hash}:0:0:RustBox:/root:/bin/rash")
    }

    #[test]
    fn parses_passwd_bcrypt_field() {
        let hash = bcrypt::hash("secret", 4).expect("hash");
        let table = parse_auth_passwd_text(&format!("{}\n# bob\n", sample_line(&hash)));
        assert!(table.check("rustbox", "secret"));
    }

    #[test]
    fn ignores_x_password_field() {
        let table = parse_auth_passwd_text("root:x:0:0:root:/root:/bin/rash\n");
        assert!(table.is_empty());
    }

    #[test]
    fn rewrites_existing_user_preserving_comment() {
        let hash = bcrypt::hash("old", 4).expect("hash");
        let new_hash = bcrypt::hash("new", 4).expect("hash2");
        let text = format!("{} # dev\n", sample_line(&hash));
        let out = rewrite_auth_passwd_text(&text, "rustbox", &new_hash);
        assert!(out.contains(&format!(
            "rustbox:{new_hash}:0:0:RustBox:/root:/bin/rash # dev"
        )));
        let table = parse_auth_passwd_text(&out);
        assert!(table.check("rustbox", "new"));
        assert!(!table.check("rustbox", "old"));
    }

    #[test]
    fn hash_with_hashmark_in_field_is_preserved() {
        let hash = "$2b$04$abcdefghijklmnopqrstuvwx#yz0123456789ABCDEFGHIJK";
        let text = format!("{}\n", sample_line(hash));
        let out = rewrite_auth_passwd_text(&text, "rustbox", "$2b$04$newhashvalue");
        assert!(out.contains("$2b$04$newhashvalue"));
        assert!(!out.contains("#yz0123"));
    }

    #[test]
    fn appends_missing_user() {
        let hash = bcrypt::hash("pw", 4).expect("hash");
        let out = rewrite_auth_passwd_text("# empty\n", "admin", &hash);
        assert!(out.contains(&format!("admin:{hash}:0:0::/root:/bin/rash")));
    }
}
