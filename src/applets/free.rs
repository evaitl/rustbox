use crate::sys;
use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    let mut human = false;
    for arg in args {
        match *arg {
            "-h" | "--human" => human = true,
            s if s.starts_with('-') => {
                usage("free", &format!("invalid option -- '{s}'"));
                return 1;
            }
            s => {
                usage("free", &format!("extra operand '{s}'"));
                return 1;
            }
        }
    }

    match sys::read_meminfo() {
        Ok(info) => {
            print_header(human);
            print_mem_line(
                "Mem:",
                info.mem_total_kb,
                info.mem_free_kb,
                info.mem_available_kb,
                info.buffers_kb + info.cached_kb,
                human,
            );
            print_mem_line(
                "Swap:",
                info.swap_total_kb,
                info.swap_free_kb,
                info.swap_free_kb,
                0,
                human,
            );
            0
        }
        Err(e) => {
            usage("free", &e.to_string());
            1
        }
    }
}

fn print_header(_human: bool) {
    println!("              total        used        free      shared  buff/cache   available");
}

fn print_mem_line(
    label: &str,
    total_kb: u64,
    free_kb: u64,
    available_kb: u64,
    cache_kb: u64,
    human: bool,
) {
    let used_kb = total_kb.saturating_sub(free_kb);
    let shared_kb = 0u64;
    if human {
        println!(
            "{label:<6} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
            format_kb(total_kb),
            format_kb(used_kb),
            format_kb(free_kb),
            format_kb(shared_kb),
            format_kb(cache_kb),
            format_kb(available_kb),
        );
    } else {
        println!(
            "{label:<6} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
            total_kb, used_kb, free_kb, shared_kb, cache_kb, available_kb
        );
    }
}

fn format_kb(kb: u64) -> String {
    const UNITS: &[&str] = &["K", "M", "G", "T"];
    let mut value = kb as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{kb}K")
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}
