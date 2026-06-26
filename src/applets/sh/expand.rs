use std::collections::HashMap;
use std::path::Path;

use super::parse::{QuoteMode, Word};
use super::Shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandError {
    Syntax,
}

pub type ExpandResult<T> = Result<T, ExpandError>;

const SYNTAX: ExpandError = ExpandError::Syntax;

pub struct ExpandCtx<'a> {
    pub vars: HashMap<String, String>,
    pub positional: &'a [String],
    pub last_status: i32,
    pub nounset: bool,
    pub assign_out: Option<&'a mut HashMap<String, String>>,
}

pub fn expand_word(ctx: &mut ExpandCtx<'_>, word: &str, split: bool) -> Result<Vec<String>, ()> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = word.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let ifs = if split { Some(ifs_chars(ctx)) } else { None };

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            continue;
        }
        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                '$' => expand_param(ctx, &mut chars, &mut current, split)?,
                _ => current.push(ch),
            }
            continue;
        }
        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '$' => expand_param(ctx, &mut chars, &mut current, split)?,
            '*' | '?' if split => {
                push_field(&mut fields, std::mem::take(&mut current));
                let mut pattern = String::from(ch);
                while let Some(&next) = chars.peek() {
                    if matches!(next, ' ' | '\t' | '\n' | '\'' | '"' | '\\' | '$') {
                        break;
                    }
                    pattern.push(chars.next().unwrap());
                }
                glob_expand(&pattern, &mut fields)?;
            }
            _ if is_ifs(ch, ifs.as_ref()) => {
                push_field(&mut fields, std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        push_field(&mut fields, current);
    } else if fields.is_empty() && !word.is_empty() && !split {
        return Ok(vec![String::new()]);
    }

    if split && fields.is_empty() {
        Ok(vec![])
    } else if split {
        Ok(fields)
    } else if fields.is_empty() {
        Ok(vec![String::new()])
    } else {
        Ok(vec![fields.join("")])
    }
}

pub fn expand_command_substitution(shell: &mut Shell, input: &str) -> Result<String, ()> {
    let mut out = String::new();
    let mut literal = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'(') {
            let mut ahead = chars.clone();
            ahead.next();
            if ahead.peek() == Some(&'(') {
                chars.next();
                chars.next();
                let expr = read_arith_expr(&mut chars)?;
                if chars.next() != Some(')') {
                    return Err(());
                }
                let ctx = shell.expand_ctx();
                let value = super::arith::eval(&expr, &ctx.vars).unwrap_or(0);
                out.push_str(&value.to_string());
                continue;
            }
            if !literal.is_empty() {
                let mut ctx = shell.expand_ctx();
                let mut assigns = HashMap::new();
                ctx.assign_out = Some(&mut assigns);
                out.push_str(&expand_literal(&mut ctx, &literal)?);
                for (k, v) in assigns {
                    shell.set_var(&k, v);
                }
                literal.clear();
            }
            chars.next();
            let inner = read_balanced(&mut chars, '(', ')')?;
            let value = shell
                .run_command_substitution(&inner)
                .map_err(|_| ())?
                .trim_end()
                .to_string();
            out.push_str(&value);
            continue;
        }
        literal.push(ch);
    }

    if !literal.is_empty() {
        let mut ctx = shell.expand_ctx();
        let mut assigns = HashMap::new();
        ctx.assign_out = Some(&mut assigns);
        out.push_str(&expand_literal(&mut ctx, &literal)?);
        for (k, v) in assigns {
            shell.set_var(&k, v);
        }
    }
    Ok(out)
}

fn expand_literal(ctx: &mut ExpandCtx<'_>, input: &str) -> Result<String, ()> {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            for c in chars.by_ref() {
                if c == '\'' {
                    break;
                }
                out.push(c);
            }
            continue;
        }
        if ch == '"' {
            while let Some(c) = chars.next() {
                if c == '"' {
                    break;
                }
                if c == '\\' {
                    if let Some(esc) = chars.next() {
                        out.push(esc);
                    }
                    continue;
                }
                if c == '$' {
                    expand_param(ctx, &mut chars, &mut out, false)?;
                    continue;
                }
                out.push(c);
            }
            continue;
        }
        if ch == '\\' {
            if let Some(esc) = chars.next() {
                out.push(esc);
            }
            continue;
        }
        if ch == '$' {
            expand_param(ctx, &mut chars, &mut out, false)?;
            continue;
        }
        out.push(ch);
    }
    Ok(out)
}

fn read_arith_expr(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, ()> {
    let mut out = String::new();
    let mut depth = 0i32;
    for ch in chars.by_ref() {
        match ch {
            '(' => {
                depth += 1;
                out.push(ch);
            }
            ')' => {
                if depth == 0 {
                    return Ok(out);
                }
                depth -= 1;
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    Err(())
}

fn read_balanced(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    open: char,
    close: char,
) -> Result<String, ()> {
    let mut depth = 1;
    let mut out = String::new();
    for ch in chars.by_ref() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Ok(out);
            }
        }
        out.push(ch);
    }
    Err(())
}

fn read_name(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, ()> {
    let mut name = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(chars.next().unwrap());
        } else {
            break;
        }
    }
    if name.is_empty() {
        Err(())
    } else {
        Ok(name)
    }
}

fn expand_param(
    ctx: &mut ExpandCtx<'_>,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
    split: bool,
) -> Result<(), ()> {
    let Some(next) = chars.next() else {
        out.push('$');
        return Ok(());
    };
    match next {
        '?' => out.push_str(&ctx.last_status.to_string()),
        '#' => out.push_str(&ctx.positional.len().saturating_sub(1).to_string()),
        '@' | '*' => {
            let items = &ctx.positional[1..];
            if split {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    out.push_str(item);
                }
            } else {
                out.push_str(&items.join(" "));
            }
        }
        '{' => {
            let name = read_name(chars)?;
            if chars.peek() == Some(&'}') {
                chars.next();
                push_var(ctx, out, &name)?;
            } else if chars.peek() == Some(&':') {
                chars.next();
                let op = chars.next().ok_or(())?;
                let word = read_brace_word(chars)?;
                if chars.next() != Some('}') {
                    return Err(());
                }
                apply_brace_subst(ctx, out, &name, op, &word, split)?;
            } else {
                return Err(());
            }
        }
        '(' => {
            if chars.peek() == Some(&'(') {
                chars.next();
                let expr = read_arith_expr(chars)?;
                if chars.next() != Some(')') {
                    return Err(());
                }
                let value = super::arith::eval(&expr, &ctx.vars)?;
                out.push_str(&value.to_string());
            } else {
                out.push('(');
            }
        }
        ch if ch.is_ascii_digit() => {
            let mut idx = ch.to_digit(10).unwrap() as usize;
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    let digit = chars.next().unwrap().to_digit(10).unwrap() as usize;
                    idx = match idx.checked_mul(10).and_then(|n| n.checked_add(digit)) {
                        Some(n) => n,
                        None => usize::MAX,
                    };
                } else {
                    break;
                }
            }
            if let Some(value) = ctx.positional.get(idx) {
                out.push_str(value);
            }
        }
        ch if ch.is_ascii_alphabetic() || ch == '_' => {
            let mut name = String::from(ch);
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            push_var(ctx, out, &name)?;
        }
        _ => {
            out.push('$');
            out.push(next);
        }
    }
    Ok(())
}

fn read_brace_word(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, ()> {
    let mut word = String::new();
    let mut depth = 0i32;
    while let Some(&ch) = chars.peek() {
        if ch == '}' && depth == 0 {
            return Ok(word);
        }
        let next = chars.next().unwrap();
        if next == '{' {
            depth += 1;
        } else if next == '}' {
            depth -= 1;
        }
        word.push(next);
    }
    Err(())
}

fn is_unset_or_null(ctx: &ExpandCtx<'_>, name: &str) -> bool {
    ctx.vars.get(name).map(|s| s.is_empty()).unwrap_or(true)
}

fn expand_brace_word(ctx: &mut ExpandCtx<'_>, word: &str, split: bool) -> Result<String, ()> {
    expand_word(ctx, word, split).map(|fields| {
        if split {
            fields.join(" ")
        } else {
            fields.concat()
        }
    })
}

fn apply_brace_subst(
    ctx: &mut ExpandCtx<'_>,
    out: &mut String,
    name: &str,
    op: char,
    word: &str,
    split: bool,
) -> Result<(), ()> {
    match op {
        '-' => {
            if is_unset_or_null(ctx, name) {
                out.push_str(&expand_brace_word(ctx, word, split)?);
            } else {
                out.push_str(ctx.vars.get(name).map(String::as_str).unwrap_or(""));
            }
        }
        '=' => {
            if is_unset_or_null(ctx, name) {
                let value = expand_brace_word(ctx, word, split)?;
                ctx.vars.insert(name.to_string(), value.clone());
                if let Some(assigns) = ctx.assign_out.as_mut() {
                    assigns.insert(name.to_string(), value.clone());
                }
                out.push_str(&value);
            } else {
                out.push_str(ctx.vars.get(name).map(String::as_str).unwrap_or(""));
            }
        }
        '+' => {
            if !is_unset_or_null(ctx, name) {
                out.push_str(&expand_brace_word(ctx, word, split)?);
            }
        }
        '?' => {
            if is_unset_or_null(ctx, name) {
                return Err(());
            }
        }
        _ => return Err(()),
    }
    Ok(())
}

fn push_var(ctx: &ExpandCtx<'_>, out: &mut String, name: &str) -> Result<(), ()> {
    match ctx.vars.get(name) {
        Some(value) => out.push_str(value),
        None if ctx.nounset => return Err(()),
        None => {}
    }
    Ok(())
}

fn push_field(fields: &mut Vec<String>, value: String) {
    if !value.is_empty() {
        fields.push(value);
    }
}

fn ifs_chars(ctx: &ExpandCtx<'_>) -> Vec<char> {
    ctx.vars
        .get("IFS")
        .map(|s| s.chars().collect())
        .unwrap_or_else(|| vec![' ', '\t', '\n'])
}

fn is_ifs(ch: char, ifs: Option<&Vec<char>>) -> bool {
    ifs.is_some_and(|set| set.contains(&ch))
}

fn glob_expand(pattern: &str, fields: &mut Vec<String>) -> Result<(), ()> {
    let (prefix, pat) = split_glob_prefix(pattern);
    let dir = if prefix.is_empty() {
        ".".to_string()
    } else if let Some(parent) = Path::new(&prefix)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        parent.to_string_lossy().into_owned()
    } else {
        ".".to_string()
    };
    let file_prefix = Path::new(&prefix)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let full_pattern = format!("{file_prefix}{pat}");

    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if glob_match(&full_pattern, &name) {
            let path = entry.path();
            matches.push(path.to_string_lossy().into_owned());
        }
    }
    matches.sort();
    if matches.is_empty() && !pattern.contains('*') && !pattern.contains('?') {
        return Ok(());
    }
    fields.extend(matches);
    Ok(())
}

fn split_glob_prefix(pattern: &str) -> (String, String) {
    if let Some(idx) = pattern.rfind(['/', '*']) {
        if pattern.as_bytes().get(idx) == Some(&b'/') {
            let (p, rest) = pattern.split_at(idx + 1);
            return (p.to_string(), rest.to_string());
        }
    }
    (String::new(), pattern.to_string())
}

pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_bytes(pattern.as_bytes(), text.as_bytes())
}

fn glob_match_bytes(pattern: &[u8], text: &[u8]) -> bool {
    let mut star_pi = None::<usize>;
    let mut star_ti = None::<usize>;
    let mut pi = 0usize;
    let mut ti = 0usize;

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == text[ti] || pattern[pi] == b'?') {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = Some(pi);
            star_ti = Some(ti);
            pi += 1;
        } else if let (Some(sp), Some(st)) = (star_pi, star_ti) {
            pi = sp + 1;
            star_ti = Some(st + 1);
            ti = st + 1;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}

pub fn expand_argv_words(shell: &mut Shell, words: &[Word]) -> ExpandResult<Vec<String>> {
    expand_argv_words_inner(shell, words).map_err(|()| SYNTAX)
}

fn expand_argv_words_inner(shell: &mut Shell, words: &[Word]) -> Result<Vec<String>, ()> {
    let mut argv = Vec::new();
    let mut assigns = HashMap::new();
    for word in words {
        if matches!(word.quote, QuoteMode::Single) {
            argv.push(word.text.clone());
            continue;
        }
        let text = expand_command_substitution(shell, &word.text)?;
        let mut ctx = shell.expand_ctx();
        ctx.assign_out = Some(&mut assigns);
        let split = matches!(word.quote, QuoteMode::None);
        argv.extend(expand_word(&mut ctx, &text, split)?);
    }
    for (k, v) in assigns {
        shell.set_var(&k, v);
    }
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_many_stars_does_not_hang() {
        assert!(glob_match("************", "ce"));
    }

    #[test]
    fn long_positional_index_does_not_overflow() {
        let vars = HashMap::new();
        let positional = vec!["rash".to_string()];
        let mut ctx = ExpandCtx {
            vars,
            positional: &positional,
            last_status: 0,
            nounset: false,
            assign_out: None,
        };
        let _ = expand_word(&mut ctx, "$99999999999999999999", false);
    }

    #[test]
    fn expands_simple_var() {
        let mut vars = HashMap::new();
        vars.insert("X".to_string(), "hi".to_string());
        let positional = vec!["rash".to_string()];
        let mut ctx = ExpandCtx {
            vars,
            positional: &positional,
            last_status: 0,
            nounset: false,
            assign_out: None,
        };
        let out = expand_word(&mut ctx, "$X", false).unwrap();
        assert_eq!(out, vec!["hi"]);
    }

    #[test]
    fn single_quoted_argv_preserves_dollar() {
        use super::super::Shell;
        let mut shell = Shell::new();
        let words = vec![super::super::parse::Word {
            text: "no $expansion".to_string(),
            quote: QuoteMode::Single,
        }];
        let argv = expand_argv_words(&mut shell, &words).unwrap();
        assert_eq!(argv, vec!["no $expansion"]);
    }

    #[test]
    fn single_quoted_argv_preserves_dollar_paren() {
        use super::super::Shell;
        let mut shell = Shell::new();
        let words = vec![super::super::parse::Word {
            text: "$(not expanded)".to_string(),
            quote: QuoteMode::Single,
        }];
        let argv = expand_argv_words(&mut shell, &words).unwrap();
        assert_eq!(argv, vec!["$(not expanded)"]);
    }

    #[test]
    fn splits_expanded_word_on_default_ifs() {
        let mut vars = HashMap::new();
        vars.insert("IFS".to_string(), " \t\n".to_string());
        let positional = vec!["rash".to_string()];
        let mut ctx = ExpandCtx {
            vars,
            positional: &positional,
            last_status: 0,
            nounset: false,
            assign_out: None,
        };
        assert_eq!(
            expand_word(&mut ctx, "a b c", true).unwrap(),
            vec!["a", "b", "c"]
        );
        assert_eq!(expand_word(&mut ctx, "a\nb", true).unwrap(), vec!["a", "b"]);
        assert_eq!(expand_word(&mut ctx, "a  b", true).unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn double_quoted_word_does_not_split_on_ifs() {
        let mut vars = HashMap::new();
        vars.insert("IFS".to_string(), " \t\n".to_string());
        let positional = vec!["rash".to_string()];
        let mut ctx = ExpandCtx {
            vars,
            positional: &positional,
            last_status: 0,
            nounset: false,
            assign_out: None,
        };
        assert_eq!(
            expand_word(&mut ctx, "a b c", false).unwrap(),
            vec!["a b c"]
        );
    }

    #[test]
    fn expands_default_substitution() {
        let vars = HashMap::new();
        let positional = vec!["rash".to_string()];
        let mut ctx = ExpandCtx {
            vars,
            positional: &positional,
            last_status: 0,
            nounset: false,
            assign_out: None,
        };
        let out = expand_word(&mut ctx, "${UNSET:-fallback}", false).unwrap();
        assert_eq!(out, vec!["fallback"]);
    }

    #[test]
    fn expands_assign_default_substitution() {
        let vars = HashMap::new();
        let positional = vec!["rash".to_string()];
        let mut assigns = HashMap::new();
        let mut ctx = ExpandCtx {
            vars,
            positional: &positional,
            last_status: 0,
            nounset: false,
            assign_out: Some(&mut assigns),
        };
        let out = expand_word(&mut ctx, "${X:=set}", false).unwrap();
        assert_eq!(out, vec!["set"]);
        assert_eq!(assigns.get("X").map(String::as_str), Some("set"));
    }

    #[test]
    fn expands_nested_default_substitution() {
        let vars = HashMap::new();
        let positional = vec!["rash".to_string()];
        let mut ctx = ExpandCtx {
            vars,
            positional: &positional,
            last_status: 0,
            nounset: false,
            assign_out: None,
        };
        let out = expand_word(&mut ctx, "${A:-${B:-inner}}", false).unwrap();
        assert_eq!(out, vec!["inner"]);
    }

    #[cfg(not(feature = "fuzzing"))]
    #[test]
    fn argv_splits_command_substitution_on_ifs() {
        use super::super::Shell;
        let mut shell = Shell::new();
        let words = vec![super::super::parse::Word {
            text: "$(echo a b c)".to_string(),
            quote: QuoteMode::None,
        }];
        let argv = expand_argv_words(&mut shell, &words).unwrap();
        assert_eq!(argv, vec!["a", "b", "c"]);
    }

    #[cfg(not(feature = "fuzzing"))]
    #[test]
    fn argv_keeps_quoted_command_substitution_unsplit() {
        use super::super::Shell;
        let mut shell = Shell::new();
        let words = vec![super::super::parse::Word {
            text: "$(echo a b c)".to_string(),
            quote: QuoteMode::Double,
        }];
        let argv = expand_argv_words(&mut shell, &words).unwrap();
        assert_eq!(argv, vec!["a b c"]);
    }

    #[test]
    fn single_quoted_empty_argv_word() {
        use super::super::Shell;
        let mut shell = Shell::new();
        let words = vec![super::super::parse::Word {
            text: String::new(),
            quote: QuoteMode::Single,
        }];
        let argv = expand_argv_words(&mut shell, &words).unwrap();
        assert_eq!(argv, vec![""]);
    }
}
