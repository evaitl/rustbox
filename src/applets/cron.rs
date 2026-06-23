use crate::sys;
use crate::{eprintln, usage};

const DEFAULT_CRONTAB: &str = "/etc/crontab";
const DEFAULT_SPOOL: &str = "/var/spool/cron/crontabs";

#[derive(Clone, Debug)]
struct CronJob {
    minute: CronField,
    hour: CronField,
    dom: CronField,
    month: CronField,
    dow: CronField,
    command: String,
}

#[derive(Clone, Debug)]
struct CronField {
    any: bool,
    values: Vec<i32>,
}

pub fn run(args: &[&str]) -> i32 {
    let mut foreground = false;
    let mut dry_run = false;
    let mut crondir: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-f" => foreground = true,
            "-b" => {}
            "-n" => dry_run = true,
            "-c" => {
                i += 1;
                if i >= args.len() {
                    usage("cron", "option requires an argument -- 'c'");
                    return 1;
                }
                crondir = Some(args[i].to_string());
            }
            s if s.starts_with('-') => {
                usage("cron", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("cron", &format!("unexpected argument -- '{s}'"));
                return 1;
            }
        }
        i += 1;
    }

    let jobs = match load_jobs(crondir.as_deref()) {
        Ok(jobs) => jobs,
        Err(e) => {
            eprintln(format!("cron: {e}"));
            return 1;
        }
    };

    if dry_run {
        return 0;
    }

    if !foreground {
        if let Err(e) = sys::daemonize() {
            eprintln(format!("cron: {e}"));
            return 1;
        }
    }

    let tz = std::env::var("TZ").unwrap_or_else(|_| "system default".into());
    eprintln(format!("cron: scheduling in local time (TZ={tz})"));

    if let Err(e) = run_daemon(&jobs) {
        eprintln(format!("cron: {e}"));
        return 1;
    }
    0
}

fn load_jobs(crondir: Option<&str>) -> Result<Vec<CronJob>, String> {
    let mut jobs = Vec::new();
    if let Some(dir) = crondir {
        load_dir(dir, false, &mut jobs)?;
        return Ok(jobs);
    }

    if sys::exists(DEFAULT_CRONTAB) {
        let text = sys::read_to_string(DEFAULT_CRONTAB)
            .map_err(|e| format!("cannot read '{DEFAULT_CRONTAB}': {e}"))?;
        parse_text(&text, true, &mut jobs)?;
    }

    if sys::is_directory(DEFAULT_SPOOL) {
        load_dir(DEFAULT_SPOOL, false, &mut jobs)?;
    }

    Ok(jobs)
}

fn load_dir(dir: &str, with_user: bool, jobs: &mut Vec<CronJob>) -> Result<(), String> {
    for entry in sys::read_dir(dir).map_err(|e| format!("cannot read '{dir}': {e}"))? {
        if entry.name.starts_with('.') {
            continue;
        }
        let path = if dir.ends_with('/') {
            format!("{dir}{}", entry.name)
        } else {
            format!("{dir}/{}", entry.name)
        };
        if entry.file_type.is_dir() {
            continue;
        }
        let text = sys::read_to_string(&path).map_err(|e| format!("cannot read '{path}': {e}"))?;
        parse_text(&text, with_user, jobs)?;
    }
    Ok(())
}

fn parse_text(text: &str, with_user: bool, jobs: &mut Vec<CronJob>) -> Result<(), String> {
    for (lineno, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        match parse_line(line, with_user) {
            Some(job) => jobs.push(job),
            None => {
                return Err(format!(
                    "invalid crontab entry at line {}: {line}",
                    lineno + 1
                ));
            }
        }
    }
    Ok(())
}

fn parse_line(line: &str, with_user: bool) -> Option<CronJob> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let (fields, command_start) = if with_user {
        if parts.len() < 7 {
            return None;
        }
        (&parts[..5], 6)
    } else if parts.len() >= 6 {
        (&parts[..5], 5)
    } else {
        return None;
    };

    Some(CronJob {
        minute: parse_field(fields[0], 0, 59)?,
        hour: parse_field(fields[1], 0, 23)?,
        dom: parse_field(fields[2], 1, 31)?,
        month: parse_field(fields[3], 1, 12)?,
        dow: parse_field(fields[4], 0, 7)?,
        command: parts[command_start..].join(" "),
    })
}

fn parse_field(field: &str, min: i32, max: i32) -> Option<CronField> {
    if field == "*" {
        return Some(CronField {
            any: true,
            values: Vec::new(),
        });
    }

    let mut values = Vec::new();
    for part in field.split(',') {
        let (base, step) = if let Some((left, right)) = part.split_once('/') {
            (left, right.parse::<i32>().ok()?)
        } else {
            (part, 1)
        };
        if step <= 0 {
            return None;
        }

        if base == "*" {
            let mut v = min;
            while v <= max {
                values.push(v);
                v += step;
            }
            continue;
        }

        let (start, end) = if let Some((a, b)) = base.split_once('-') {
            (a.parse::<i32>().ok()?, b.parse::<i32>().ok()?)
        } else {
            let n = base.parse::<i32>().ok()?;
            (n, n)
        };

        if start < min || end > max || start > end {
            return None;
        }

        let mut v = start;
        while v <= end {
            values.push(v);
            v += step;
        }
    }

    values.sort_unstable();
    values.dedup();
    Some(CronField { any: false, values })
}

impl CronField {
    fn matches(&self, value: i32) -> bool {
        if self.any {
            return true;
        }
        let normalized = if value == 7 { 0 } else { value };
        self.values.iter().any(|&v| {
            let field = if v == 7 { 0 } else { v };
            field == normalized
        })
    }
}

impl CronJob {
    fn matches(&self, now: &CronTime) -> bool {
        self.minute.matches(now.minute)
            && self.hour.matches(now.hour)
            && self.dom.matches(now.dom)
            && self.month.matches(now.month)
            && self.dow.matches(now.dow)
    }
}

struct CronTime {
    minute: i32,
    hour: i32,
    dom: i32,
    month: i32,
    dow: i32,
    stamp: i64,
}

fn local_now() -> CronTime {
    unsafe {
        let secs = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::MaybeUninit::<libc::tm>::zeroed();
        if libc::localtime_r(&secs, tm.as_mut_ptr()).is_null() {
            return utc_fallback(secs);
        }
        let tm = tm.assume_init();
        CronTime {
            minute: tm.tm_min,
            hour: tm.tm_hour,
            dom: tm.tm_mday,
            month: tm.tm_mon + 1,
            dow: tm.tm_wday,
            stamp: secs / 60,
        }
    }
}

fn utc_fallback(secs: libc::time_t) -> CronTime {
    let minute = ((secs / 60) % 60) as i32;
    let hour = ((secs / 3600) % 24) as i32;
    let days = secs / 86_400;
    let (_year, month, dom) = civil_from_days(days);
    let dow = ((days + 4) % 7) as i32;
    CronTime {
        minute,
        hour,
        dom,
        month,
        dow,
        stamp: secs / 60,
    }
}

fn civil_from_days(z: i64) -> (i32, i32, i32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp as i32 + if mp < 10 { 3 } else { -9 };
    let y = y as i32 + if m <= 2 { 1 } else { 0 };
    (y, m, d as i32)
}

fn run_daemon(jobs: &[CronJob]) -> sys::Result<()> {
    let mut last_stamp = -1;
    loop {
        let _ = sys::reap_zombies()?;
        let now = local_now();
        if now.stamp != last_stamp {
            last_stamp = now.stamp;
            for job in jobs {
                if job.matches(&now) {
                    let _ = sys::spawn(&job.command);
                }
            }
        }
        sys::sleep_seconds(1.0)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_crontab_line() {
        let job = parse_line("*/5 * * * * /bin/true", false).unwrap();
        assert!(job.minute.matches(0));
        assert!(job.minute.matches(5));
        assert!(!job.minute.matches(3));
    }

    #[test]
    fn parses_etc_crontab_with_user() {
        let job = parse_line("0 2 * * * root /sbin/backup", true).unwrap();
        assert_eq!(job.command, "/sbin/backup");
        assert!(job.hour.matches(2));
    }
}
