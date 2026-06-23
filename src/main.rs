use anyhow::Result;
use rustbox::{applet_names, find_applet, invocation_name};
use std::env;
use std::process;

fn print_help() {
    println!("RustBox - a BusyBox-style multi-call binary");
    println!();
    println!("Usage:");
    println!("  rustbox <applet> [arguments...]");
    println!("  <applet> [arguments...]          (via symlink)");
    println!();
    println!("Options:");
    println!("  --help    Show this help");
    println!("  --list    List available applets");
}

fn print_applets() {
    let mut names: Vec<_> = applet_names().collect();
    names.sort_unstable();
    for name in names {
        println!("{name}");
    }
}

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    let name = invocation_name(args.first().map(String::as_str).unwrap_or("rustbox"));

    let (applet_name, applet_args) = if name == "rustbox" {
        match argv.get(1).copied() {
            None | Some("--help") | Some("-h") => {
                print_help();
                return Ok(());
            }
            Some("--list") => {
                print_applets();
                return Ok(());
            }
            Some(applet) => (applet, &argv[2..]),
        }
    } else {
        (name.as_str(), &argv[1..])
    };

    let Some(run) = find_applet(applet_name) else {
        eprintln!("rustbox: applet not found: {applet_name}");
        eprintln!("Try 'rustbox --list' to see available applets.");
        process::exit(127);
    };

    let code = run(applet_args);
    if code != 0 {
        process::exit(code);
    }
    Ok(())
}
