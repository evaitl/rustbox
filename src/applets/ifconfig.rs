use crate::net::iface::{
    self, configure_interface, format_ifconfig_line, get_if_info, list_interface_names,
};
use crate::net::ipv4::parse_ipv4;
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() || args == ["-a"] {
        return show_all();
    }

    let iface = args[0];
    let mut i = 1;

    if i >= args.len() {
        return show_one(iface);
    }

    let mut addr: Option<&str> = None;
    let mut netmask: Option<&str> = None;
    let mut up = false;
    let mut down = false;

    while i < args.len() {
        match args[i] {
            "up" => up = true,
            "down" => down = true,
            "netmask" => {
                i += 1;
                if i >= args.len() {
                    usage("ifconfig", "netmask requires an argument");
                    return 1;
                }
                netmask = Some(args[i]);
            }
            s if parse_ipv4(s).is_some() => addr = Some(s),
            s => {
                usage("ifconfig", &format!("unknown argument: {s}"));
                return 1;
            }
        }
        i += 1;
    }

    if let Some(ip) = addr {
        if let Err(e) = configure_interface(iface, ip, netmask, up || !down) {
            eprintln(format!("ifconfig: {e}"));
            return 1;
        }
    } else if up {
        if let Err(e) = iface::set_if_up(iface, true) {
            eprintln(format!("ifconfig: {e}"));
            return 1;
        }
    } else if down {
        if let Err(e) = iface::set_if_up(iface, false) {
            eprintln(format!("ifconfig: {e}"));
            return 1;
        }
    }
    0
}

fn show_all() -> i32 {
    match list_interface_names() {
        Ok(names) => {
            for name in names {
                let _ = show_one(&name);
            }
            0
        }
        Err(e) => {
            eprintln(format!("ifconfig: {e}"));
            1
        }
    }
}

fn show_one(name: &str) -> i32 {
    match get_if_info(name) {
        Ok(info) => {
            print!("{}", format_ifconfig_line(&info));
            0
        }
        Err(e) => {
            eprintln(format!("ifconfig: {name}: {e}"));
            1
        }
    }
}
