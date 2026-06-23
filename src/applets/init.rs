use crate::sys;
use crate::{eprintln, usage};
use std::collections::HashMap;

const DEFAULT_INITTAB: &str = "/etc/inittab";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Sysinit,
    Respawn,
    Once,
    Wait,
}

#[derive(Clone, Debug)]
struct Entry {
    id: String,
    action: Action,
    command: String,
}

pub fn run(args: &[&str]) -> i32 {
    let mut inittab = DEFAULT_INITTAB.to_string();
    let mut oneshot = false;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" => {
                i += 1;
                if i >= args.len() {
                    usage("init", "option requires an argument -- 'f'");
                    return 1;
                }
                inittab = args[i].to_string();
            }
            "-s" | "--oneshot" => oneshot = true,
            s if s.starts_with('-') => {
                usage("init", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("init", &format!("unexpected argument -- '{s}'"));
                return 1;
            }
        }
        i += 1;
    }

    if !sys::is_init_process() {
        eprintln("init: warning: not running as pid 1");
    }

    let entries = match load_inittab(&inittab) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln(format!("init: cannot read '{inittab}': {e}"));
            return 1;
        }
    };

    if entries.is_empty() {
        usage("init", "no valid entries in inittab");
        return 1;
    }

    if let Err(e) = run_entries(&entries, oneshot) {
        eprintln(format!("init: {e}"));
        return 1;
    }
    0
}

fn load_inittab(path: &str) -> sys::Result<Vec<Entry>> {
    let text = sys::read_to_string(path)?;
    Ok(parse_inittab(&text))
}

fn parse_inittab(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some((id, action, command)) = parse_line(line) else {
            continue;
        };
        let Some(action) = parse_action(action) else {
            continue;
        };
        if command.is_empty() {
            continue;
        }
        entries.push(Entry {
            id: id.to_string(),
            action,
            command: command.to_string(),
        });
    }
    entries
}

fn parse_line(line: &str) -> Option<(&str, &str, &str)> {
    let mut parts = line.splitn(4, ':');
    let id = parts.next()?;
    let _runlevel = parts.next()?;
    let action = parts.next()?;
    let command = parts.next()?;
    Some((id, action, command))
}

fn parse_action(action: &str) -> Option<Action> {
    match action {
        "sysinit" => Some(Action::Sysinit),
        "respawn" => Some(Action::Respawn),
        "once" => Some(Action::Once),
        "wait" => Some(Action::Wait),
        _ => None,
    }
}

fn respawn_console_id(id: &str) -> bool {
    matches!(id, "con" | "console" | "serial")
}

fn is_tty_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("tty") else {
        return false;
    };
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn spawn_respawn_entry(entry: &Entry) -> sys::Result<rustix::process::Pid> {
    if is_tty_id(&entry.id) {
        let device = format!("/dev/{}", entry.id);
        sys::spawn_respawn_on(&entry.command, &device)
    } else if respawn_console_id(&entry.id) {
        sys::spawn_respawn(&entry.command)
    } else {
        sys::spawn(&entry.command)
    }
}

fn run_entries(entries: &[Entry], oneshot: bool) -> sys::Result<()> {
    for entry in entries.iter().filter(|e| e.action == Action::Sysinit) {
        run_and_wait(entry)?;
    }

    for entry in entries.iter().filter(|e| e.action == Action::Wait) {
        run_and_wait(entry)?;
    }

    for entry in entries.iter().filter(|e| e.action == Action::Once) {
        let _ = sys::spawn(&entry.command)?;
    }

    if oneshot {
        return Ok(());
    }

    let respawn_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.action == Action::Respawn)
        .map(|(i, _)| i)
        .collect();

    let mut respawn_by_pid = HashMap::new();
    for index in respawn_indices {
        let pid = spawn_respawn_entry(&entries[index])?;
        respawn_by_pid.insert(pid, index);
    }

    loop {
        match sys::wait_any()? {
            Some((pid, _status)) => {
                if let Some(index) = respawn_by_pid.remove(&pid) {
                    let new_pid = spawn_respawn_entry(&entries[index])?;
                    respawn_by_pid.insert(new_pid, index);
                }
            }
            None => {
                if respawn_by_pid.is_empty() {
                    sys::sleep_seconds(0.25)?;
                }
            }
        }
    }
}

fn run_and_wait(entry: &Entry) -> sys::Result<()> {
    let pid = sys::spawn(&entry.command)?;
    let _ = sys::wait_pid(pid)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inittab_lines() {
        let text = r#"
# comment
::sysinit:/sbin/boot
tty1::respawn:/sbin/getty
::once:/bin/true
::wait:/bin/setup
::off:/bin/ignored
"#;
        let entries = parse_inittab(text);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].action, Action::Sysinit);
        assert_eq!(entries[0].command, "/sbin/boot");
        assert_eq!(entries[1].id, "tty1");
        assert_eq!(entries[1].action, Action::Respawn);
        assert_eq!(entries[2].action, Action::Once);
        assert_eq!(entries[3].action, Action::Wait);
    }

    #[test]
    fn console_respawn_ids() {
        assert!(respawn_console_id("con"));
        assert!(respawn_console_id("console"));
        assert!(!respawn_console_id(""));
        assert!(is_tty_id("tty1"));
        assert!(is_tty_id("ttyAMA0"));
        assert!(!is_tty_id("con"));
        assert!(!is_tty_id("tty"));
    }
}
