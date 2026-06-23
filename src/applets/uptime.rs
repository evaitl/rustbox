use crate::sys;
use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    if let Some(arg) = args.first() {
        if arg.starts_with('-') {
            usage("uptime", &format!("invalid option -- '{arg}'"));
            return 1;
        }
        usage("uptime", "extra operand");
        return 1;
    }

    match sys::read_uptime() {
        Ok(info) => {
            println!(
                "{} load average: {:.2}, {:.2}, {:.2}",
                format_uptime(info.uptime_secs),
                info.load_1,
                info.load_5,
                info.load_15
            );
            0
        }
        Err(e) => {
            usage("uptime", &e.to_string());
            1
        }
    }
}

fn format_uptime(secs: f64) -> String {
    let total = secs as u64;
    let days = total / 86400;
    let hours = (total % 86400) / 3600;
    let mins = (total % 3600) / 60;
    if days > 0 {
        let day_label = if days == 1 { "day" } else { "days" };
        format!("up {days} {day_label}, {hours}:{mins:02}")
    } else if hours > 0 {
        format!("up {hours}:{mins:02}")
    } else {
        let min_label = if mins == 1 { "min" } else { "mins" };
        format!("up {mins} {min_label}")
    }
}
