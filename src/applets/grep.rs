use crate::sys;
use crate::{eprintln, usage};
use rustix::stdio;

struct Options {
    ignore_case: bool,
    invert: bool,
    line_number: bool,
    quiet: bool,
    fixed: bool,
    word: bool,
    whole_line: bool,
    count: bool,
    files_only: bool,
    with_filename: bool,
    no_filename: bool,
    recursive: bool,
    patterns: Vec<String>,
    paths: Vec<String>,
}

pub fn run(args: &[&str]) -> i32 {
    let mut opts = Options {
        ignore_case: false,
        invert: false,
        line_number: false,
        quiet: false,
        fixed: false,
        word: false,
        whole_line: false,
        count: false,
        files_only: false,
        with_filename: false,
        no_filename: false,
        recursive: false,
        patterns: Vec::new(),
        paths: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-i" => opts.ignore_case = true,
            "-v" => opts.invert = true,
            "-n" => opts.line_number = true,
            "-q" => opts.quiet = true,
            "-F" => opts.fixed = true,
            "-w" => opts.word = true,
            "-x" => opts.whole_line = true,
            "-c" => opts.count = true,
            "-l" => opts.files_only = true,
            "-H" => opts.with_filename = true,
            "-h" => opts.no_filename = true,
            "-r" | "-R" => opts.recursive = true,
            "-e" => {
                i += 1;
                if i >= args.len() {
                    usage("grep", "option requires an argument -- 'e'");
                    return 2;
                }
                opts.patterns.push(args[i].to_string());
            }
            s if s.starts_with('-') && s.len() > 1 => {
                usage("grep", &format!("invalid option -- '{s}'"));
                return 2;
            }
            s => {
                if opts.patterns.is_empty() {
                    opts.patterns.push(s.to_string());
                } else {
                    opts.paths.push(s.to_string());
                }
            }
        }
        i += 1;
    }

    if opts.patterns.is_empty() {
        usage("grep", "pattern missing");
        return 2;
    }

    if opts.paths.is_empty() {
        return grep_fd(stdio::stdin(), "-", &opts, true);
    }

    let show_name =
        (opts.paths.len() > 1 || opts.recursive || opts.with_filename) && !opts.no_filename;
    let mut status = 1;
    for path in &opts.paths {
        let code = grep_path(path, &opts, show_name);
        if code == 0 {
            status = 0;
        } else if code > 1 {
            return code;
        }
    }
    status
}

fn grep_path(path: &str, opts: &Options, show_name: bool) -> i32 {
    if opts.recursive && sys::is_directory(path) {
        return grep_dir(path, opts, show_name);
    }
    match sys::open_read(path) {
        Ok(fd) => grep_fd(fd, path, opts, show_name),
        Err(e) => {
            eprintln(format!("grep: {path}: {e}"));
            2
        }
    }
}

fn grep_dir(dir: &str, opts: &Options, show_name: bool) -> i32 {
    let mut status = 1;
    match sys::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let child = join_path(dir, &entry.name);
                let code = if entry.file_type.is_dir() {
                    grep_dir(&child, opts, show_name)
                } else {
                    match sys::open_read(&child) {
                        Ok(fd) => grep_fd(fd, &child, opts, show_name),
                        Err(e) => {
                            eprintln(format!("grep: {child}: {e}"));
                            2
                        }
                    }
                };
                if code == 0 {
                    status = 0;
                } else if code > 1 {
                    return code;
                }
            }
        }
        Err(e) => {
            eprintln(format!("grep: {dir}: {e}"));
            return 2;
        }
    }
    status
}

fn grep_fd<Fd: rustix::fd::AsFd>(fd: Fd, label: &str, opts: &Options, show_name: bool) -> i32 {
    let mut matched = false;
    let mut match_count = 0u64;
    let mut line_no = 0u64;

    let result = sys::for_each_line(fd, |line| {
        line_no += 1;
        let text = String::from_utf8_lossy(line);
        let line_text = text.strip_suffix('\n').unwrap_or(&text);
        let hit = opts
            .patterns
            .iter()
            .any(|pat| line_matches(line_text, pat, opts));
        if hit != opts.invert {
            matched = true;
            match_count += 1;
            if !opts.quiet && !opts.files_only && !opts.count {
                print_match(label, line_no, line_text, show_name, opts);
            }
        }
        true
    });

    if let Err(e) = result {
        eprintln(format!("grep: {label}: read error: {e}"));
        return 2;
    }

    if matched {
        if !opts.quiet {
            if opts.files_only {
                println!("{label}");
            } else if opts.count {
                if show_name {
                    println!("{match_count}:{label}");
                } else {
                    println!("{match_count}");
                }
            }
        }
        0
    } else {
        1
    }
}

fn print_match(label: &str, line_no: u64, line: &str, show_name: bool, opts: &Options) {
    let mut prefix = String::new();
    if show_name {
        prefix.push_str(label);
        prefix.push(':');
    }
    if opts.line_number {
        prefix.push_str(&line_no.to_string());
        prefix.push(':');
    }
    if prefix.is_empty() {
        println!("{line}");
    } else {
        println!("{prefix}{line}");
    }
}

fn line_matches(line: &str, pattern: &str, opts: &Options) -> bool {
    if opts.whole_line {
        return eq_fold(line, pattern, opts.ignore_case);
    }
    if opts.word {
        return find_word(line, pattern, opts);
    }
    if opts.fixed || !pattern.contains(['.', '*', '[', '^', '$']) {
        return find_substr(line, pattern, opts.ignore_case);
    }
    match_bre(line, pattern, opts.ignore_case)
}

fn eq_fold(a: &str, b: &str, ignore_case: bool) -> bool {
    if ignore_case {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

fn find_substr(hay: &str, needle: &str, ignore_case: bool) -> bool {
    if !ignore_case {
        return hay.contains(needle);
    }
    let hay: Vec<char> = hay.chars().collect();
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() {
        return true;
    }
    if needle.len() > hay.len() {
        return false;
    }
    'outer: for start in 0..=hay.len() - needle.len() {
        for (i, &nc) in needle.iter().enumerate() {
            if !hay[start + i].eq_ignore_ascii_case(&nc) {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

fn find_word(line: &str, pattern: &str, opts: &Options) -> bool {
    let bytes = line.as_bytes();
    let plen = pattern.len();
    if plen == 0 {
        return true;
    }
    let mut i = 0;
    while i + plen <= bytes.len() {
        if line.is_char_boundary(i) && line.is_char_boundary(i + plen) {
            let slice = &line[i..i + plen];
            if eq_fold(slice, pattern, opts.ignore_case)
                && !has_word_char_before(line, i)
                && !has_word_char_after(line, i + plen)
            {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn has_word_char_before(line: &str, idx: usize) -> bool {
    line[..idx]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn has_word_char_after(line: &str, idx: usize) -> bool {
    line[idx..]
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn match_bre(line: &str, pattern: &str, ignore_case: bool) -> bool {
    match compile_bre(pattern) {
        Ok(re) => BreMatcher { prog: re }.is_match(line, ignore_case),
        Err(()) => find_substr(line, pattern, ignore_case),
    }
}

#[derive(Debug)]
enum BreInsn {
    Lit(char),
    Any,
    Start,
    End,
    Class(Vec<char>, bool),
    Star(Box<BreInsn>),
    Plus(Box<BreInsn>),
    Quest(Box<BreInsn>),
    Alt(Vec<BreProgram>),
}

type BreProgram = Vec<BreInsn>;

fn compile_bre(pattern: &str) -> Result<BreProgram, ()> {
    let mut chars = pattern.chars().peekable();
    let prog = compile_branch(&mut chars)?;
    if chars.next().is_some() {
        return Err(());
    }
    if prog.is_empty() {
        return Err(());
    }
    Ok(prog)
}

fn compile_branch(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<BreProgram, ()> {
    let mut alts = vec![compile_piece(chars)?];
    while chars.peek() == Some(&'|') {
        chars.next();
        alts.push(compile_piece(chars)?);
    }
    if alts.len() == 1 {
        Ok(alts.into_iter().next().unwrap())
    } else {
        Ok(vec![BreInsn::Alt(alts)])
    }
}

fn compile_piece(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<BreProgram, ()> {
    let mut prog = Vec::new();
    while let Some(&ch) = chars.peek() {
        if ch == '|' || ch == ')' {
            break;
        }
        if ch == '^' {
            chars.next();
            prog.push(BreInsn::Start);
            continue;
        }
        if ch == '$' {
            chars.next();
            prog.push(BreInsn::End);
            continue;
        }
        if ch == '.' {
            chars.next();
            let mut insn = BreInsn::Any;
            insn = postfix(chars, insn)?;
            prog.push(insn);
            continue;
        }
        if ch == '[' {
            chars.next();
            let (set, neg) = parse_class(chars)?;
            let mut insn = BreInsn::Class(set, neg);
            insn = postfix(chars, insn)?;
            prog.push(insn);
            continue;
        }
        if ch == '(' {
            chars.next();
            let inner = compile_branch(chars)?;
            if chars.next() != Some(')') {
                return Err(());
            }
            let mut insn = BreInsn::Alt(vec![inner]);
            insn = postfix(chars, insn)?;
            prog.push(insn);
            continue;
        }
        if ch == '*' || ch == '+' || ch == '?' {
            return Err(());
        }
        if ch == '\\' {
            chars.next();
            let lit = chars.next().ok_or(())?;
            let mut insn = BreInsn::Lit(lit);
            insn = postfix(chars, insn)?;
            prog.push(insn);
            continue;
        }
        chars.next();
        let mut insn = BreInsn::Lit(ch);
        insn = postfix(chars, insn)?;
        prog.push(insn);
    }
    Ok(prog)
}

fn postfix(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    insn: BreInsn,
) -> Result<BreInsn, ()> {
    match chars.peek() {
        Some('*') => {
            chars.next();
            Ok(BreInsn::Star(Box::new(insn)))
        }
        Some('+') => {
            chars.next();
            Ok(BreInsn::Plus(Box::new(insn)))
        }
        Some('?') => {
            chars.next();
            Ok(BreInsn::Quest(Box::new(insn)))
        }
        _ => Ok(insn),
    }
}

fn parse_class(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<(Vec<char>, bool), ()> {
    let mut neg = false;
    if chars.peek() == Some(&'^') {
        neg = true;
        chars.next();
    }
    let mut set = Vec::new();
    while let Some(ch) = chars.next() {
        if ch == ']' {
            return Ok((set, neg));
        }
        if ch == '\\' {
            set.push(chars.next().ok_or(())?);
        } else {
            set.push(ch);
        }
    }
    Err(())
}

struct BreMatcher {
    prog: BreProgram,
}

impl BreMatcher {
    fn is_match(&self, line: &str, ignore_case: bool) -> bool {
        let chars: Vec<char> = line.chars().collect();
        (0..=chars.len()).any(|start| self.match_here(&self.prog, &chars, start, ignore_case))
    }

    fn match_here(&self, prog: &[BreInsn], chars: &[char], pos: usize, ignore_case: bool) -> bool {
        let mut p = pos;
        for insn in prog {
            match insn {
                BreInsn::Start => {
                    if p != 0 {
                        return false;
                    }
                }
                BreInsn::End => {
                    if p != chars.len() {
                        return false;
                    }
                }
                BreInsn::Alt(alts) => {
                    return alts
                        .iter()
                        .any(|alt| self.match_here(alt, chars, p, ignore_case));
                }
                BreInsn::Star(inner) => {
                    let mut cur = p;
                    loop {
                        if self.match_rest(&prog[1..], chars, cur, ignore_case) {
                            return true;
                        }
                        match self.try_match_one(inner, chars, cur, ignore_case) {
                            Ok(next) if next > cur => cur = next,
                            _ => return false,
                        }
                    }
                }
                BreInsn::Plus(inner) => {
                    let (ok, next) = self.match_one(inner, chars, p, ignore_case);
                    if !ok {
                        return false;
                    }
                    p = next;
                    let mut cur = p;
                    loop {
                        if self.match_rest(&prog[1..], chars, cur, ignore_case) {
                            return true;
                        }
                        match self.try_match_one(inner, chars, cur, ignore_case) {
                            Ok(next) if next > cur => cur = next,
                            _ => return false,
                        }
                    }
                }
                BreInsn::Quest(inner) => {
                    if let Ok(next) = self.try_match_one(inner, chars, p, ignore_case) {
                        if self.match_rest(&prog[1..], chars, next, ignore_case) {
                            return true;
                        }
                    }
                    return self.match_rest(&prog[1..], chars, p, ignore_case);
                }
                other => {
                    let (ok, next) = self.match_one(other, chars, p, ignore_case);
                    if !ok {
                        return false;
                    }
                    p = next;
                }
            }
        }
        true
    }

    fn match_rest(&self, prog: &[BreInsn], chars: &[char], pos: usize, ignore_case: bool) -> bool {
        if prog.is_empty() {
            return true;
        }
        self.match_here(prog, chars, pos, ignore_case)
    }

    fn match_one(
        &self,
        insn: &BreInsn,
        chars: &[char],
        pos: usize,
        ignore_case: bool,
    ) -> (bool, usize) {
        match self.try_match_one(insn, chars, pos, ignore_case) {
            Ok(np) => (true, np),
            Err(()) => (false, pos),
        }
    }

    fn try_match_one(
        &self,
        insn: &BreInsn,
        chars: &[char],
        pos: usize,
        ignore_case: bool,
    ) -> Result<usize, ()> {
        if pos > chars.len() {
            return Err(());
        }
        match insn {
            BreInsn::Lit(c) => {
                let ch = chars.get(pos).copied().ok_or(())?;
                if char_eq(ch, *c, ignore_case) {
                    Ok(pos + 1)
                } else {
                    Err(())
                }
            }
            BreInsn::Any => {
                if pos < chars.len() {
                    Ok(pos + 1)
                } else {
                    Err(())
                }
            }
            BreInsn::Class(set, neg) => {
                let ch = chars.get(pos).copied().ok_or(())?;
                let in_set = set.iter().any(|&c| char_eq(ch, c, ignore_case));
                if in_set != *neg {
                    Ok(pos + 1)
                } else {
                    Err(())
                }
            }
            BreInsn::Alt(alts) => {
                for alt in alts {
                    if let Ok(np) = self.match_prog_end(alt, chars, pos, ignore_case) {
                        return Ok(np);
                    }
                }
                Err(())
            }
            _ => Err(()),
        }
    }

    fn match_prog_end(
        &self,
        prog: &[BreInsn],
        chars: &[char],
        pos: usize,
        ignore_case: bool,
    ) -> Result<usize, ()> {
        let mut p = pos;
        for insn in prog {
            let (ok, np) = self.match_one(insn, chars, p, ignore_case);
            if !ok {
                return Err(());
            }
            p = np;
        }
        Ok(p)
    }
}

fn char_eq(a: char, b: char, ignore_case: bool) -> bool {
    if ignore_case {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}

fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_substring_match() {
        let opts = Options {
            ignore_case: false,
            invert: false,
            line_number: false,
            quiet: false,
            fixed: true,
            word: false,
            whole_line: false,
            count: false,
            files_only: false,
            with_filename: false,
            no_filename: false,
            recursive: false,
            patterns: vec!["ell".to_string()],
            paths: vec![],
        };
        assert!(line_matches("hello", "ell", &opts));
        assert!(!line_matches("hello", "xyz", &opts));
    }

    #[test]
    fn bre_dot_star() {
        let opts = Options {
            ignore_case: false,
            invert: false,
            line_number: false,
            quiet: false,
            fixed: false,
            word: false,
            whole_line: false,
            count: false,
            files_only: false,
            with_filename: false,
            no_filename: false,
            recursive: false,
            patterns: vec!["h.llo".to_string()],
            paths: vec![],
        };
        assert!(line_matches("hello", "h.llo", &opts));
    }
}
