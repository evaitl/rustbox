use crate::usage;

pub fn run(args: &[&str]) -> i32 {
    if args.is_empty() {
        usage("printf", "missing format operand");
        return 1;
    }

    let format = args[0];
    let mut values = args[1..].iter();
    let mut out = String::new();
    let mut chars = format.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        let Some(spec) = chars.next() else {
            usage("printf", "%: missing format character");
            return 1;
        };
        match spec {
            '%' => out.push('%'),
            's' => out.push_str(values.next().copied().unwrap_or("")),
            'c' => {
                let arg = values.next().copied().unwrap_or("");
                let ch = arg.chars().next().unwrap_or('\0');
                out.push(ch);
            }
            'd' | 'i' => match parse_i64(values.next().copied().unwrap_or("0")) {
                Ok(n) => out.push_str(&n.to_string()),
                Err(()) => {
                    usage("printf", "invalid number");
                    return 1;
                }
            },
            'u' => match parse_u64(values.next().copied().unwrap_or("0")) {
                Ok(n) => out.push_str(&n.to_string()),
                Err(()) => {
                    usage("printf", "invalid number");
                    return 1;
                }
            },
            'x' => match parse_u64(values.next().copied().unwrap_or("0")) {
                Ok(n) => out.push_str(&format!("{n:x}")),
                Err(()) => {
                    usage("printf", "invalid number");
                    return 1;
                }
            },
            'X' => match parse_u64(values.next().copied().unwrap_or("0")) {
                Ok(n) => out.push_str(&format!("{n:X}")),
                Err(()) => {
                    usage("printf", "invalid number");
                    return 1;
                }
            },
            'o' => match parse_u64(values.next().copied().unwrap_or("0")) {
                Ok(n) => out.push_str(&format!("{n:o}")),
                Err(()) => {
                    usage("printf", "invalid number");
                    return 1;
                }
            },
            _ => {
                usage("printf", &format!("invalid format character '%{spec}'"));
                return 1;
            }
        }
    }

    print!("{out}");
    0
}

fn parse_i64(s: &str) -> Result<i64, ()> {
    s.parse::<i64>().map_err(|_| ())
}

fn parse_u64(s: &str) -> Result<u64, ()> {
    if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(rest, 16).map_err(|_| ())
    } else {
        s.parse::<u64>().map_err(|_| ())
    }
}
