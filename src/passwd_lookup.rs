use rustix::process::{geteuid, Gid, Uid};
use std::ffi::CString;
use std::fs;
use std::io;
use std::path::Path;

pub const DEFAULT_PASSWD: &str = "/etc/passwd";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub name: String,
    pub uid: Uid,
    pub gid: Gid,
    pub home: String,
    pub shell: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupError {
    NoPasswd,
    UnknownUser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivDropError {
    NoPasswd,
    UnknownUser(String),
    Os(String),
}

impl std::fmt::Display for PrivDropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPasswd => write!(f, "{DEFAULT_PASSWD}: no such file"),
            Self::UnknownUser(name) => write!(f, "unknown user {name}"),
            Self::Os(err) => write!(f, "{err}"),
        }
    }
}

pub fn drop_to_user(name_or_uid: &str) -> Result<(), PrivDropError> {
    if name_or_uid.is_empty() || !geteuid().is_root() {
        return Ok(());
    }
    let user = lookup_user(name_or_uid, Path::new(DEFAULT_PASSWD)).map_err(|err| match err {
        LookupError::NoPasswd => PrivDropError::NoPasswd,
        LookupError::UnknownUser => PrivDropError::UnknownUser(name_or_uid.to_string()),
    })?;
    drop_user(&user)
}

pub fn drop_user(user: &User) -> Result<(), PrivDropError> {
    let cname = CString::new(user.name.as_str())
        .map_err(|_| PrivDropError::Os("invalid username".to_string()))?;
    if unsafe { libc::initgroups(cname.as_ptr(), user.gid.as_raw()) } != 0 {
        return Err(PrivDropError::Os(io::Error::last_os_error().to_string()));
    }
    if unsafe { libc::setgid(user.gid.as_raw()) } != 0 {
        return Err(PrivDropError::Os(io::Error::last_os_error().to_string()));
    }
    if unsafe { libc::setuid(user.uid.as_raw()) } != 0 {
        return Err(PrivDropError::Os(io::Error::last_os_error().to_string()));
    }
    Ok(())
}

pub fn lookup_user(name_or_uid: &str, passwd_path: &Path) -> Result<User, LookupError> {
    let text = fs::read_to_string(passwd_path).map_err(|_| LookupError::NoPasswd)?;

    if let Ok(uid) = name_or_uid.parse::<u32>() {
        if let Some(user) = find_by_uid(&text, uid) {
            return Ok(user);
        }
        return Ok(User {
            name: name_or_uid.to_string(),
            uid: Uid::from_raw(uid),
            gid: Gid::from_raw(uid),
            home: "/".to_string(),
            shell: "/bin/rash".to_string(),
        });
    }

    find_by_name(&text, name_or_uid).ok_or(LookupError::UnknownUser)
}

pub fn parse_passwd_text(text: &str) -> Vec<User> {
    text.lines().filter_map(parse_passwd_line).collect()
}

fn find_by_name(text: &str, name: &str) -> Option<User> {
    text.lines()
        .filter_map(parse_passwd_line)
        .find(|user| user.name == name)
}

fn find_by_uid(text: &str, uid: u32) -> Option<User> {
    text.lines()
        .filter_map(parse_passwd_line)
        .find(|user| user.uid.as_raw() == uid)
}

fn parse_passwd_line(line: &str) -> Option<User> {
    let line = line.split('#').next()?.trim();
    if line.is_empty() {
        return None;
    }
    let fields: Vec<&str> = line.split(':').collect();
    if fields.len() < 7 {
        return None;
    }
    let uid = fields[2].parse().ok()?;
    let gid = fields[3].parse().ok()?;
    Some(User {
        name: fields[0].to_string(),
        uid: Uid::from_raw(uid),
        gid: Gid::from_raw(gid),
        home: fields[5].to_string(),
        shell: fields[6].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"root:x:0:0:root:/root:/bin/rash
nobody:x:65534:65534:nobody:/:/bin/false
daemon:x:2:2:daemon:/:/bin/rash
"#;

    #[test]
    fn parses_passwd_entries() {
        let users = parse_passwd_text(SAMPLE);
        assert_eq!(users.len(), 3);
        assert_eq!(users[0].name, "root");
        assert_eq!(users[1].uid.as_raw(), 65534);
    }

    #[test]
    fn looks_up_by_name() {
        let text_path = std::env::temp_dir().join(format!("rustbox-passwd-{}", std::process::id()));
        std::fs::write(&text_path, SAMPLE).unwrap();
        let user = lookup_user("nobody", &text_path).unwrap();
        assert_eq!(user.uid.as_raw(), 65534);
        let _ = std::fs::remove_file(text_path);
    }

    #[test]
    fn missing_passwd_file_errors() {
        let missing =
            std::env::temp_dir().join(format!("rustbox-passwd-missing-{}", std::process::id()));
        let err = lookup_user("nobody", &missing).unwrap_err();
        assert_eq!(err, LookupError::NoPasswd);
    }

    #[test]
    fn looks_up_numeric_uid_without_entry() {
        let text_path =
            std::env::temp_dir().join(format!("rustbox-passwd-uid-{}", std::process::id()));
        std::fs::write(&text_path, SAMPLE).unwrap();
        let user = lookup_user("4242", &text_path).unwrap();
        assert_eq!(user.uid.as_raw(), 4242);
        assert_eq!(user.gid.as_raw(), 4242);
        let _ = std::fs::remove_file(text_path);
    }
}
