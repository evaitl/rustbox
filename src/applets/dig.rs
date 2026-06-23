use crate::net::dns_query::{build_query, parse_qtype, query_udp, reverse_ipv4_name};
use crate::net::ipv4::format_ipv4;
use crate::{eprintln, usage};
use simple_dns::rdata::RData;
use simple_dns::{Packet, QTYPE, RCODE, TYPE};
use std::net::Ipv6Addr;
use std::time::Instant;

const DEFAULT_SERVER: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 53;

pub fn run(args: &[&str]) -> i32 {
    let mut server = DEFAULT_SERVER.to_string();
    let mut port = DEFAULT_PORT;
    let mut qtype = QTYPE::TYPE(TYPE::A);
    let mut reverse = false;
    let mut name: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == "-x" {
            reverse = true;
        } else if arg == "-t" {
            i += 1;
            if i >= args.len() {
                usage("dig", "option requires an argument -- 't'");
                return 1;
            }
            qtype = match parse_qtype(args[i]) {
                Some(t) => t,
                None => {
                    usage("dig", "unknown query type");
                    return 1;
                }
            };
        } else if arg == "-p" {
            i += 1;
            if i >= args.len() {
                usage("dig", "option requires an argument -- 'p'");
                return 1;
            }
            port = match args[i].parse() {
                Ok(p) if p > 0 => p,
                _ => {
                    usage("dig", "invalid port");
                    return 1;
                }
            };
        } else if let Some(rest) = arg.strip_prefix('@') {
            server = rest.to_string();
        } else if arg.starts_with('-') {
            usage("dig", &format!("invalid option -- '{arg}'"));
            return 1;
        } else {
            name = Some(arg.to_string());
        }
        i += 1;
    }

    let name = match name {
        Some(n) => n,
        None => {
            usage("dig", "usage: dig [@server] [-p port] [-t type] [-x] name");
            return 1;
        }
    };

    let qname = if reverse {
        match reverse_ipv4_name(&name) {
            Some(n) => {
                qtype = QTYPE::TYPE(TYPE::PTR);
                n
            }
            None => {
                usage("dig", "invalid IPv4 address for reverse lookup");
                return 1;
            }
        }
    } else {
        name
    };

    let query = match build_query(&qname, qtype) {
        Ok(q) => q,
        Err(_) => {
            eprintln("dig: invalid query name");
            return 1;
        }
    };

    let start = Instant::now();
    let response = match query_udp(&server, port, &query, 5000) {
        Ok(r) => r,
        Err(e) => {
            eprintln(format!("dig: {server}#{port}: {e}"));
            return 1;
        }
    };
    let elapsed_ms = start.elapsed().as_millis();
    let response_len = response.len();

    let packet = match Packet::parse(&response) {
        Ok(p) => p,
        Err(_) => {
            eprintln("dig: malformed response");
            return 1;
        }
    };

    print_response(&qname, &server, port, &packet, elapsed_ms, response_len);
    if packet.rcode() == RCODE::NoError {
        0
    } else {
        1
    }
}

fn print_response(
    qname: &str,
    server: &str,
    port: u16,
    packet: &Packet<'_>,
    elapsed_ms: u128,
    response_len: usize,
) {
    println!(";; QUESTION SECTION:");
    println!(";{qname}.\t\tIN\t{}\n", query_type_name(packet));

    if !packet.answers.is_empty() {
        println!(";; ANSWER SECTION:");
        for rr in &packet.answers {
            print_record(rr);
        }
        println!();
    }

    if !packet.name_servers.is_empty() {
        println!(";; AUTHORITY SECTION:");
        for rr in &packet.name_servers {
            print_record(rr);
        }
        println!();
    }

    if !packet.additional_records.is_empty() {
        println!(";; ADDITIONAL SECTION:");
        for rr in &packet.additional_records {
            print_record(rr);
        }
        println!();
    }

    println!(";; Query time: {elapsed_ms} msec");
    println!(";; SERVER: {server}#{port}({server})");
    println!(";; MSG SIZE  rcvd: {response_len}");
}

fn print_record(rr: &simple_dns::ResourceRecord<'_>) {
    println!(
        "{}\t{}\t{:?}\t{:?}\t{}",
        rr.name,
        rr.ttl,
        rr.class,
        rr.rdata.type_code(),
        format_rdata(rr)
    );
}

fn query_type_name(packet: &Packet<'_>) -> String {
    packet
        .questions
        .first()
        .map(|q| format!("{:?}", q.qtype))
        .unwrap_or_else(|| "A".to_string())
}

fn format_rdata(rr: &simple_dns::ResourceRecord<'_>) -> String {
    match &rr.rdata {
        RData::A(a) => format_ipv4(a.address),
        RData::AAAA(aaaa) => Ipv6Addr::from(aaaa.address).to_string(),
        RData::NS(ns) => ns.0.to_string(),
        RData::CNAME(cname) => cname.0.to_string(),
        RData::PTR(ptr) => ptr.0.to_string(),
        RData::MX(mx) => format!("{} {}", mx.preference, mx.exchange),
        RData::TXT(txt) => format!("{txt:?}"),
        _ => format!("{:?}", rr.rdata),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_name() {
        assert_eq!(
            reverse_ipv4_name("192.168.1.1").as_deref(),
            Some("1.1.168.192.in-addr.arpa")
        );
    }

    #[test]
    fn formats_ipv4_rdata() {
        assert_eq!(
            format_ipv4(u32::from_be_bytes([93, 184, 216, 34])),
            "93.184.216.34"
        );
    }
}
