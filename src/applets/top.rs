use crate::sys;
use crate::usage;
use std::io::{self, Write};

pub fn run(args: &[&str]) -> i32 {
    let mut delay = 3u64;
    let mut iterations = usize::MAX;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-n" => {
                i += 1;
                if i >= args.len() {
                    usage("top", "option requires an argument -- 'n'");
                    return 1;
                }
                iterations = match args[i].parse() {
                    Ok(0) => usize::MAX,
                    Ok(n) => n,
                    Err(_) => {
                        usage("top", "invalid number of iterations");
                        return 1;
                    }
                };
            }
            "-d" => {
                i += 1;
                if i >= args.len() {
                    usage("top", "option requires an argument -- 'd'");
                    return 1;
                }
                delay = match args[i].parse() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        usage("top", "invalid delay");
                        return 1;
                    }
                };
            }
            "-h" | "--help" => {
                usage("top", "usage: top [-n COUNT] [-d SEC]");
                return 0;
            }
            s if s.starts_with('-') => {
                usage("top", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("top", &format!("unexpected argument -- '{s}'"));
                return 1;
            }
        }
        i += 1;
    }

    let mut stdout = io::stdout();
    for round in 0..iterations {
        if round > 0 {
            let _ = sys::sleep_seconds(delay as f64);
        }
        if let Err(e) = render_snapshot(&mut stdout) {
            usage("top", &e.to_string());
            return 1;
        }
        let _ = stdout.flush();
    }
    0
}

fn render_snapshot(stdout: &mut io::Stdout) -> io::Result<()> {
    let procs = sys::list_processes().map_err(io::Error::other)?;
    let mut rows: Vec<(u32, u64, String)> = procs
        .into_iter()
        .map(|p| {
            let rss = sys::proc_rss_kb(p.pid).unwrap_or(0);
            (p.pid, rss, p.comm)
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    writeln!(stdout, "\x1b[H\x1b[J")?;
    writeln!(stdout, "  PID   RSS COMMAND")?;
    for (pid, rss, comm) in rows {
        writeln!(stdout, "{pid:5} {rss:5}K {comm}")?;
    }
    Ok(())
}
