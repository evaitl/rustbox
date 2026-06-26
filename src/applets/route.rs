use crate::net::ipv4::parse_ipv4;
use crate::net::route::{
    self, add_default_gateway, add_host_route, add_net_route, del_default_gateway, del_host_route,
    del_net_route, format_routes, read_routes,
};
use crate::{eprintln, usage};

pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() {
        return show_table();
    }

    match args[0] {
        "add" => route_modify(&args[1..], true),
        "del" | "delete" => route_modify(&args[1..], false),
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

struct RouteArgs<'a> {
    host: bool,
    net: bool,
    target: &'a str,
    gw: Option<u32>,
    dev: Option<&'a str>,
}

fn parse_route_args<'a>(args: &'a [&str]) -> Result<RouteArgs<'a>, i32> {
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
        return Err(1);
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
                    return Err(1);
                }
                gw = parse_ipv4(args[i]);
                if gw.is_none() {
                    usage("route", "invalid gateway");
                    return Err(1);
                }
            }
            "dev" => {
                i += 1;
                if i >= args.len() {
                    usage("route", "dev requires an interface");
                    return Err(1);
                }
                dev = Some(args[i]);
            }
            "default" if target == "default" => {}
            s => {
                usage("route", &format!("unknown argument: {s}"));
                return Err(1);
            }
        }
        i += 1;
    }

    Ok(RouteArgs {
        host,
        net,
        target,
        gw,
        dev,
    })
}

fn route_modify(args: &[&str], add: bool) -> i32 {
    let spec = match parse_route_args(args) {
        Ok(spec) => spec,
        Err(code) => return code,
    };

    let result = if spec.target == "default" {
        let Some(gateway) = spec.gw else {
            usage("route", "default route requires gw");
            return 1;
        };
        let iface = spec.dev.unwrap_or("eth0");
        if add {
            add_default_gateway(gateway, iface)
        } else {
            del_default_gateway(gateway, iface)
        }
    } else if spec.host {
        let (dst, _) = match route::parse_route_target(spec.target, true) {
            Ok(v) => v,
            Err(e) => {
                eprintln(format!("route: {e}"));
                return 1;
            }
        };
        if add {
            add_host_route(dst, spec.gw, spec.dev)
        } else {
            del_host_route(dst, spec.gw, spec.dev)
        }
    } else if spec.net {
        let ip = parse_ipv4(spec.target).unwrap_or(0);
        let mask = parse_ipv4("255.255.255.0").unwrap_or(0xffffff00);
        if add {
            add_net_route(ip, mask, spec.gw, spec.dev)
        } else {
            del_net_route(ip, mask, spec.gw, spec.dev)
        }
    } else {
        let (dst, len) = match route::parse_route_target(spec.target, false) {
            Ok(v) => v,
            Err(e) => {
                eprintln(format!("route: {e}"));
                return 1;
            }
        };
        if add {
            route::add_route(dst, len, spec.gw, spec.dev)
        } else {
            route::del_route(dst, len, spec.gw, spec.dev)
        }
    };

    if let Err(e) = result {
        eprintln(format!("route: {e}"));
        1
    } else {
        0
    }
}
