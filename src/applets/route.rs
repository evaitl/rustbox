use crate::net::ipv4::parse_ipv4;
use crate::net::route::{
    self, add_default_gateway, add_host_route, add_net_route, format_routes, read_routes,
};
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() {
        return show_table();
    }

    match args[0] {
        "add" => route_add(&args[1..]),
        "del" | "delete" => route_del(&args[1..]),
        "-n" => show_table(),
        s if parse_ipv4(s).is_some() => {
            usage("route", "legacy syntax not supported; use: route add ...");
            1
        }
        s => {
            usage("route", &format!("unknown command: {s}"));
            1
        }
    }
}

fn show_table() -> i32 {
    match read_routes() {
        Ok(routes) => {
            print!("{}", format_routes(&routes));
            0
        }
        Err(e) => {
            eprintln(format!("route: {e}"));
            1
        }
    }
}

fn route_add(args: &[&str]) -> i32 {
    let mut host = false;
    let mut net = false;
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-host" => host = true,
            "-net" => net = true,
            _ => break,
        }
        i += 1;
    }
    if i >= args.len() {
        usage("route", "missing target");
        return 1;
    }
    let target = args[i];
    i += 1;

    let mut gw: Option<u32> = None;
    let mut dev: Option<&str> = None;

    while i < args.len() {
        match args[i] {
            "gw" | "gateway" => {
                i += 1;
                if i >= args.len() {
                    usage("route", "gw requires an address");
                    return 1;
                }
                gw = parse_ipv4(args[i]);
                if gw.is_none() {
                    usage("route", "invalid gateway");
                    return 1;
                }
            }
            "dev" => {
                i += 1;
                if i >= args.len() {
                    usage("route", "dev requires an interface");
                    return 1;
                }
                dev = Some(args[i]);
            }
            "default" if target == "default" => {}
            s => {
                usage("route", &format!("unknown argument: {s}"));
                return 1;
            }
        }
        i += 1;
    }

    let result = if target == "default" {
        let Some(gateway) = gw else {
            usage("route", "default route requires gw");
            return 1;
        };
        let iface = dev.unwrap_or("eth0");
        add_default_gateway(gateway, iface)
    } else if host {
        let (dst, _) = match route::parse_route_target(target, true) {
            Ok(v) => v,
            Err(e) => {
                eprintln(format!("route: {e}"));
                return 1;
            }
        };
        add_host_route(dst, gw, dev)
    } else if net {
        let ip = parse_ipv4(target).unwrap_or(0);
        let mask = parse_ipv4("255.255.255.0").unwrap_or(0xffffff00);
        add_net_route(ip, mask, gw, dev)
    } else {
        let (dst, len) = match route::parse_route_target(target, false) {
            Ok(v) => v,
            Err(e) => {
                eprintln(format!("route: {e}"));
                return 1;
            }
        };
        route::add_route(dst, len, gw, dev)
    };

    if let Err(e) = result {
        eprintln(format!("route: {e}"));
        1
    } else {
        0
    }
}

fn route_del(_args: &[&str]) -> i32 {
    usage("route", "route del is not implemented");
    1
}
