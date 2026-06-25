#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Redirect {
    Input(String),
    Output(String),
    Append(String),
    ErrOutput(String),
    DupOut(String),
    ErrToOut,
    HereDoc {
        delimiter: String,
        quoted: bool,
        body: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteMode {
    None,
    Single,
    Double,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub quote: QuoteMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleCommand {
    pub assigns: Vec<(String, String)>,
    pub words: Vec<Word>,
    pub redirects: Vec<Redirect>,
}

#[derive(Clone, Debug)]
pub enum Command {
    Simple(SimpleCommand),
    If {
        cond: List,
        then_part: List,
        elifs: Vec<(List, List)>,
        else_part: Option<List>,
    },
    While {
        cond: List,
        body: List,
    },
    For {
        var: String,
        items: Vec<String>,
        has_in: bool,
        body: List,
    },
    Case {
        word: Word,
        arms: Vec<CaseArm>,
    },
    FunctionDef {
        name: String,
        body: List,
    },
    Brace(List),
    Subshell(List),
}

#[derive(Clone, Debug)]
pub struct CaseArm {
    pub patterns: Vec<String>,
    pub body: List,
}

#[derive(Clone, Debug)]
pub struct Pipeline {
    pub negated: bool,
    pub commands: Vec<Command>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AndOrOp {
    And,
    Or,
}

#[derive(Clone, Debug)]
pub struct AndOr {
    pub pipelines: Vec<Pipeline>,
    pub ops: Vec<AndOrOp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListSep {
    Semi,
    Background,
    Newline,
}

#[derive(Clone, Debug)]
pub struct List {
    pub andors: Vec<AndOr>,
    pub seps: Vec<ListSep>,
}

#[derive(Debug)]
pub enum ParseError {
    UnexpectedEof,
    Expected(&'static str),
    Syntax,
}

pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
    depth: usize,
}

const MAX_PARSE_DEPTH: usize = 256;

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            depth: 0,
        }
    }

    fn with_depth<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        if self.depth >= MAX_PARSE_DEPTH {
            return Err(ParseError::Syntax);
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    pub fn parse_list(&mut self) -> Result<List, ParseError> {
        self.parse_list_until(&[])
    }

    fn parse_list_until(&mut self, terminators: &[&str]) -> Result<List, ParseError> {
        self.skip_list_separators();
        if self.at_end() || self.peek_keyword(terminators) {
            return Ok(empty_list());
        }
        let mut andors = vec![self.parse_andor()?];
        let mut seps = Vec::new();
        loop {
            self.skip_blank();
            if self.at_end() || self.peek_keyword(terminators) {
                break;
            }
            if self.peek_char() == Some('#') {
                self.skip_line();
                self.skip_blank();
                if self.at_end() || self.peek_keyword(terminators) {
                    break;
                }
            }
            let sep = if self.consume_char(';') {
                ListSep::Semi
            } else if self.consume_char('&') {
                if !self.consume_char('&') {
                    ListSep::Background
                } else {
                    return Err(ParseError::Syntax);
                }
            } else if self.consume_newline() {
                ListSep::Newline
            } else {
                break;
            };
            self.skip_list_separators();
            if self.at_end() || self.peek_keyword(terminators) {
                seps.push(sep);
                break;
            }
            seps.push(sep);
            andors.push(self.parse_andor()?);
        }
        Ok(List { andors, seps })
    }

    fn parse_andor(&mut self) -> Result<AndOr, ParseError> {
        let mut pipelines = vec![self.parse_pipeline()?];
        let mut ops = Vec::new();
        loop {
            self.skip_blank();
            if self.consume_str("&&") {
                ops.push(AndOrOp::And);
            } else if self.consume_str("||") {
                ops.push(AndOrOp::Or);
            } else {
                break;
            }
            pipelines.push(self.parse_pipeline()?);
        }
        Ok(AndOr { pipelines, ops })
    }

    fn parse_pipeline(&mut self) -> Result<Pipeline, ParseError> {
        self.skip_blank();
        let negated = self.consume_char('!');
        self.skip_blank();
        let mut commands = vec![self.parse_command()?];
        loop {
            self.skip_blank();
            if self.peek_char() == Some('|') {
                let saved = self.pos;
                self.bump();
                if self.peek_char() == Some('|') {
                    self.pos = saved;
                    break;
                }
                self.pos = saved;
            }
            if !self.consume_char('|') {
                break;
            }
            commands.push(self.parse_command()?);
        }
        Ok(Pipeline { negated, commands })
    }

    fn parse_command(&mut self) -> Result<Command, ParseError> {
        self.with_depth(|p| p.parse_command_inner())
    }

    fn parse_command_inner(&mut self) -> Result<Command, ParseError> {
        self.skip_blank();
        if self.consume_char('{') {
            let list = self.parse_list_until(&["}"])?;
            self.skip_list_separators();
            if !self.consume_char('}') {
                return Err(ParseError::Expected("}"));
            }
            return Ok(Command::Brace(list));
        }
        if self.consume_char('(') {
            let list = self.parse_list_until(&[")"])?;
            self.skip_list_separators();
            if !self.consume_char(')') {
                return Err(ParseError::Expected(")"));
            }
            return Ok(Command::Subshell(list));
        }
        if let Some(word) = self.peek_word()? {
            match word.as_str() {
                "if" => return self.parse_if(),
                "while" => return self.parse_while(),
                "for" => return self.parse_for(),
                "case" => return self.parse_case(),
                "function" => return self.parse_function_keyword(),
                _ => {
                    if let Some(name) = word.strip_suffix("()").filter(|n| is_name(n)) {
                        self.next_word()?;
                        self.skip_blank();
                        if !self.consume_char('{') {
                            return Err(ParseError::Expected("{"));
                        }
                        let body = self.parse_list_until(&["}"])?;
                        self.skip_list_separators();
                        if !self.consume_char('}') {
                            return Err(ParseError::Expected("}"));
                        }
                        return Ok(Command::FunctionDef {
                            name: name.to_string(),
                            body,
                        });
                    }
                    if let Some(name) = word.strip_suffix('(').filter(|n| is_name(n)) {
                        self.next_word()?;
                        self.skip_blank();
                        if !self.consume_char(')') {
                            return Err(ParseError::Expected(")"));
                        }
                        self.skip_blank();
                        if !self.consume_char('{') {
                            return Err(ParseError::Expected("{"));
                        }
                        let body = self.parse_list_until(&["}"])?;
                        self.skip_list_separators();
                        if !self.consume_char('}') {
                            return Err(ParseError::Expected("}"));
                        }
                        return Ok(Command::FunctionDef {
                            name: name.to_string(),
                            body,
                        });
                    }
                    if is_name(&word) {
                        let saved = self.pos;
                        self.next_word()?;
                        self.skip_blank();
                        if self.consume_char('(') {
                            self.skip_blank();
                            if !self.consume_char(')') {
                                return Err(ParseError::Expected(")"));
                            }
                            self.skip_blank();
                            if !self.consume_char('{') {
                                return Err(ParseError::Expected("{"));
                            }
                            let body = self.parse_list_until(&["}"])?;
                            self.skip_list_separators();
                            if !self.consume_char('}') {
                                return Err(ParseError::Expected("}"));
                            }
                            return Ok(Command::FunctionDef { name: word, body });
                        }
                        self.pos = saved;
                    }
                }
            }
        }
        self.parse_simple_command()
    }

    fn parse_if(&mut self) -> Result<Command, ParseError> {
        self.expect_word("if")?;
        let cond = self.parse_list_until(&["then"])?;
        self.expect_word("then")?;
        let then_part = self.parse_list_until(&["elif", "else", "fi"])?;
        let mut elifs = Vec::new();
        while self.peek_keyword(&["elif"]) {
            self.expect_word("elif")?;
            let econd = self.parse_list_until(&["then"])?;
            self.expect_word("then")?;
            let ebody = self.parse_list_until(&["elif", "else", "fi"])?;
            elifs.push((econd, ebody));
        }
        let else_part = if self.peek_keyword(&["else"]) {
            self.expect_word("else")?;
            Some(self.parse_list_until(&["fi"])?)
        } else {
            None
        };
        self.expect_word("fi")?;
        Ok(Command::If {
            cond,
            then_part,
            elifs,
            else_part,
        })
    }

    fn parse_while(&mut self) -> Result<Command, ParseError> {
        self.expect_word("while")?;
        let cond = self.parse_list_until(&["do"])?;
        self.expect_word("do")?;
        let body = self.parse_list_until(&["done"])?;
        self.expect_word("done")?;
        Ok(Command::While { cond, body })
    }

    fn parse_for(&mut self) -> Result<Command, ParseError> {
        self.expect_word("for")?;
        let var = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
        if !is_name(&var) {
            return Err(ParseError::Syntax);
        }
        let mut items = Vec::new();
        let mut has_in = false;
        if self.peek_word()? == Some("in".to_string()) {
            has_in = true;
            self.expect_word("in")?;
            loop {
                self.skip_blank();
                if at_keyword(self) {
                    break;
                }
                if let Some(word) = self.next_word()? {
                    items.push(word.text);
                } else {
                    break;
                }
            }
        }
        self.skip_blank();
        if self.consume_char(';') {
            self.skip_blank();
        }
        self.expect_word("do")?;
        let body = self.parse_list_until(&["done"])?;
        self.expect_word("done")?;
        Ok(Command::For {
            var,
            items,
            has_in,
            body,
        })
    }

    fn parse_function_keyword(&mut self) -> Result<Command, ParseError> {
        self.expect_word("function")?;
        let name = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
        if !is_name(&name) {
            return Err(ParseError::Syntax);
        }
        self.skip_blank();
        if self.consume_char('(') {
            self.skip_blank();
            if !self.consume_char(')') {
                return Err(ParseError::Expected(")"));
            }
            self.skip_blank();
        }
        if !self.consume_char('{') {
            return Err(ParseError::Expected("{"));
        }
        let body = self.parse_list_until(&["}"])?;
        self.skip_list_separators();
        if !self.consume_char('}') {
            return Err(ParseError::Expected("}"));
        }
        Ok(Command::FunctionDef { name, body })
    }

    fn parse_case(&mut self) -> Result<Command, ParseError> {
        self.expect_word("case")?;
        let word = self.next_word()?.ok_or(ParseError::UnexpectedEof)?;
        self.expect_word("in")?;
        let mut arms = Vec::new();
        loop {
            self.skip_list_separators();
            if self.peek_keyword(&["esac"]) {
                break;
            }
            let mut patterns = Vec::new();
            loop {
                self.skip_blank();
                let pattern = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
                patterns.push(pattern);
                self.skip_blank();
                if self.consume_char('|') {
                    continue;
                }
                break;
            }
            self.skip_blank();
            if !self.consume_char(')') {
                return Err(ParseError::Expected(")"));
            }
            let body = self.parse_case_arm_body()?;
            arms.push(CaseArm { patterns, body });
        }
        self.expect_word("esac")?;
        Ok(Command::Case { word, arms })
    }

    fn parse_case_arm_body(&mut self) -> Result<List, ParseError> {
        self.skip_list_separators();
        if self.consume_str(";;") {
            return Ok(empty_list());
        }
        let mut andors = vec![self.parse_andor()?];
        let mut seps = Vec::new();
        loop {
            self.skip_blank();
            if self.consume_str(";;") {
                break;
            }
            if self.at_end() {
                return Err(ParseError::UnexpectedEof);
            }
            let sep = if self.consume_char(';') {
                ListSep::Semi
            } else if self.consume_char('&') {
                if !self.consume_char('&') {
                    ListSep::Background
                } else {
                    return Err(ParseError::Syntax);
                }
            } else if self.consume_newline() {
                ListSep::Newline
            } else {
                break;
            };
            self.skip_list_separators();
            if self.consume_str(";;") {
                seps.push(sep);
                break;
            }
            seps.push(sep);
            andors.push(self.parse_andor()?);
        }
        Ok(List { andors, seps })
    }

    fn parse_simple_command(&mut self) -> Result<Command, ParseError> {
        let mut assigns = Vec::new();
        let mut words = Vec::new();
        let mut redirects = Vec::new();
        loop {
            self.skip_blank();
            if at_stop(self) {
                break;
            }
            if let Some(redir) = self.try_redirect()? {
                redirects.push(redir);
                continue;
            }
            let Some(word) = self.next_word()? else {
                break;
            };
            if words.is_empty() && assigns.is_empty() && is_assignment(&word.text) {
                if let Some((name, value)) = split_assignment(&word.text) {
                    assigns.push((name, value));
                    continue;
                }
            }
            words.push(word);
        }
        if assigns.is_empty() && words.is_empty() && redirects.is_empty() {
            return Err(ParseError::Syntax);
        }
        Ok(Command::Simple(SimpleCommand {
            assigns,
            words,
            redirects,
        }))
    }

    fn try_redirect(&mut self) -> Result<Option<Redirect>, ParseError> {
        let start = self.pos;
        self.skip_blank();
        if self.consume_str("2>&1") {
            return Ok(Some(Redirect::ErrToOut));
        }
        if self.consume_str("<<-") {
            let (delimiter, quoted) = self.parse_heredoc_delimiter()?;
            let body = self.read_heredoc_body(&delimiter, true)?;
            return Ok(Some(Redirect::HereDoc {
                delimiter,
                quoted,
                body,
            }));
        }
        if self.consume_str("<<") {
            let (delimiter, quoted) = self.parse_heredoc_delimiter()?;
            let body = self.read_heredoc_body(&delimiter, false)?;
            return Ok(Some(Redirect::HereDoc {
                delimiter,
                quoted,
                body,
            }));
        }
        if self.consume_str(">>") {
            let target = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
            return Ok(Some(Redirect::Append(target)));
        }
        if self.consume_str("2>") {
            let target = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
            return Ok(Some(Redirect::ErrOutput(target)));
        }
        if self.consume_str(">&") {
            let target = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
            return Ok(Some(Redirect::DupOut(target)));
        }
        if self.consume_char('<') {
            let target = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
            return Ok(Some(Redirect::Input(target)));
        }
        if self.consume_char('>') {
            let target = self.next_word()?.ok_or(ParseError::UnexpectedEof)?.text;
            return Ok(Some(Redirect::Output(target)));
        }
        self.pos = start;
        Ok(None)
    }

    fn parse_heredoc_delimiter(&mut self) -> Result<(String, bool), ParseError> {
        self.skip_blank();
        if self.peek_char() == Some('\'') {
            self.bump();
            let mut delim = String::new();
            while let Some(ch) = self.peek_char() {
                self.bump();
                if ch == '\'' {
                    break;
                }
                delim.push(ch);
            }
            return Ok((delim, true));
        }
        if self.peek_char() == Some('"') {
            self.bump();
            let mut delim = String::new();
            while let Some(ch) = self.peek_char() {
                if ch == '"' {
                    self.bump();
                    break;
                }
                if ch == '\\' {
                    self.bump();
                    if let Some(esc) = self.peek_char() {
                        self.bump();
                        delim.push(esc);
                    }
                    continue;
                }
                self.bump();
                delim.push(ch);
            }
            return Ok((delim, true));
        }
        let word = self.next_word()?.ok_or(ParseError::UnexpectedEof)?;
        Ok((word.text, false))
    }

    fn read_heredoc_body(&mut self, delim: &str, strip_tabs: bool) -> Result<String, ParseError> {
        self.skip_blank();
        if self.peek_char() == Some('\n') {
            self.bump();
        }
        let mut body = String::new();
        loop {
            if self.at_end() {
                return Err(ParseError::UnexpectedEof);
            }
            let line_start = self.pos;
            let mut line = String::new();
            while let Some(ch) = self.peek_char() {
                if ch == '\n' {
                    self.bump();
                    break;
                }
                self.bump();
                line.push(ch);
            }
            let line_for_match = if strip_tabs {
                line.trim_start_matches('\t')
            } else {
                line.as_str()
            };
            if line_for_match == delim {
                break;
            }
            if !body.is_empty() || !line.is_empty() {
                body.push_str(&line);
                body.push('\n');
            }
            if self.at_end() && line_start == self.pos {
                return Err(ParseError::UnexpectedEof);
            }
        }
        Ok(body)
    }

    fn expect_word(&mut self, expected: &'static str) -> Result<(), ParseError> {
        while matches!(self.peek_char(), Some(' ') | Some('\t') | Some('\n')) {
            self.bump();
        }
        match self.next_word()? {
            Some(word) if word.text == expected => Ok(()),
            _ => Err(ParseError::Expected(expected)),
        }
    }

    fn peek_word(&mut self) -> Result<Option<String>, ParseError> {
        let pos = self.pos;
        let word = self.next_word()?;
        self.pos = pos;
        Ok(word.map(|w| w.text))
    }

    fn next_word(&mut self) -> Result<Option<Word>, ParseError> {
        self.skip_blank();
        if at_stop(self) {
            return Ok(None);
        }
        let ch = match self.peek_char() {
            Some(c) => c,
            None => return Ok(None),
        };
        if ch == ';' || ch == '|' || ch == '&' || ch == ')' || ch == '}' || ch == '\n' {
            return Ok(None);
        }
        if ch == '>' || ch == '<' {
            return Ok(None);
        }
        self.read_word()
    }

    fn read_word(&mut self) -> Result<Option<Word>, ParseError> {
        self.skip_blank();
        let mut quote = QuoteMode::None;
        let mut out = String::new();
        let mut started = false;
        let mut expand = None::<ExpandCtx>;
        if self.peek_char() == Some('\'') {
            quote = QuoteMode::Single;
        } else if self.peek_char() == Some('"') {
            quote = QuoteMode::Double;
        }
        while let Some(ch) = self.peek_char() {
            if !started {
                if ch == ';' || ch == '|' || ch == '&' || ch == ')' || ch == '}' || ch == '\n' {
                    break;
                }
                if ch == '>' || ch == '<' {
                    break;
                }
                if ch == ' ' || ch == '\t' {
                    self.bump();
                    continue;
                }
            }
            match ch {
                ' ' | '\t' | '\n' | ';' | '|' | '&' | '}' if started && expand.is_none() => break,
                ')' if started && expand.is_none() => break,
                '>' | '<' if started => break,
                '\'' => {
                    self.bump();
                    started = true;
                    quote = QuoteMode::Single;
                    while let Some(c) = self.peek_char() {
                        self.bump();
                        if c == '\'' {
                            break;
                        }
                        out.push(c);
                    }
                }
                '"' => {
                    self.bump();
                    started = true;
                    quote = QuoteMode::Double;
                    while let Some(c) = self.peek_char() {
                        if c == '"' {
                            self.bump();
                            break;
                        }
                        if c == '\\' {
                            self.bump();
                            if let Some(esc) = self.peek_char() {
                                self.bump();
                                out.push(esc);
                            }
                            continue;
                        }
                        self.bump();
                        out.push(c);
                    }
                }
                '\\' => {
                    self.bump();
                    if let Some(esc) = self.peek_char() {
                        self.bump();
                        out.push(esc);
                        started = true;
                    }
                }
                '#' if !started => break,
                '$' => {
                    self.bump();
                    out.push('$');
                    started = true;
                    if self.peek_char() == Some('(') {
                        self.bump();
                        out.push('(');
                        if self.peek_char() == Some('(') {
                            self.bump();
                            out.push('(');
                            expand = Some(ExpandCtx::Arith { depth: 0 });
                        } else {
                            expand = Some(ExpandCtx::CmdSub { depth: 1 });
                        }
                    }
                }
                '(' if expand.is_some() => {
                    self.bump();
                    out.push('(');
                    match &mut expand {
                        Some(ExpandCtx::Arith { depth }) | Some(ExpandCtx::CmdSub { depth }) => {
                            *depth += 1;
                        }
                        None => {}
                    }
                }
                ')' if started => match &mut expand {
                    Some(ExpandCtx::Arith { depth }) => {
                        self.bump();
                        out.push(')');
                        if *depth == 0 {
                            if self.peek_char() == Some(')') {
                                self.bump();
                                out.push(')');
                                expand = None;
                            }
                        } else {
                            *depth -= 1;
                        }
                    }
                    Some(ExpandCtx::CmdSub { depth }) => {
                        self.bump();
                        out.push(')');
                        *depth -= 1;
                        if *depth == 0 {
                            expand = None;
                        }
                    }
                    None => break,
                },
                _ => {
                    self.bump();
                    out.push(ch);
                    started = true;
                }
            }
        }
        if started || !out.is_empty() {
            Ok(Some(Word { text: out, quote }))
        } else {
            Ok(None)
        }
    }

    fn skip_blank(&mut self) -> bool {
        let mut moved = false;
        while let Some(ch) = self.peek_char() {
            if ch == ' ' || ch == '\t' {
                self.bump();
                moved = true;
            } else {
                break;
            }
        }
        moved
    }

    fn skip_list_separators(&mut self) {
        loop {
            if self.skip_blank() {
                continue;
            }
            if self.consume_char('\n') {
                continue;
            }
            if self.peek_char() == Some('#') {
                self.skip_line();
                continue;
            }
            break;
        }
    }

    fn skip_line(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.pos += ch.len_utf8();
            if ch == '\n' {
                break;
            }
        }
    }

    fn consume_newline(&mut self) -> bool {
        self.skip_blank();
        if self.peek_char() == Some('\n') {
            self.bump();
            true
        } else {
            false
        }
    }

    fn bump(&mut self) {
        if let Some(ch) = self.peek_char() {
            self.pos += ch.len_utf8();
        }
    }

    fn consume_char(&mut self, ch: char) -> bool {
        if self.peek_char() == Some(ch) {
            self.pos += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn consume_str(&mut self, s: &str) -> bool {
        if self.input[self.pos..].starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }
}

fn empty_list() -> List {
    List {
        andors: Vec::new(),
        seps: Vec::new(),
    }
}

enum ExpandCtx {
    CmdSub { depth: i32 },
    Arith { depth: i32 },
}

fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    !name.is_empty() && is_name(name)
}

fn split_assignment(word: &str) -> Option<(String, String)> {
    let (name, value) = word.split_once('=')?;
    if name.is_empty() || !is_name(name) {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

fn peek_keyword(parser: &Parser<'_>, keys: &[&str]) -> bool {
    if keys.is_empty() {
        return false;
    }
    let mut pos = parser.pos;
    while let Some(ch) = parser.input[pos..].chars().next() {
        if ch == ' ' || ch == '\t' || ch == '\n' {
            pos += ch.len_utf8();
        } else {
            break;
        }
    }
    let rest = &parser.input[pos..];
    for key in keys {
        if let Some(stripped) = rest.strip_prefix(key) {
            let next = stripped.chars().next();
            if next.is_none() || matches!(next, Some(' ' | '\t' | '\n' | ';')) {
                return true;
            }
        }
    }
    false
}

impl Parser<'_> {
    fn peek_keyword(&self, keys: &[&str]) -> bool {
        peek_keyword(self, keys)
    }
}
fn at_keyword(parser: &Parser<'_>) -> bool {
    peek_keyword(
        parser,
        &[
            "do", "done", "then", "else", "elif", "fi", "in", "esac", ";;",
        ],
    )
}

fn at_stop(parser: &Parser<'_>) -> bool {
    if parser.at_end() {
        return true;
    }
    matches!(
        parser.peek_char(),
        Some(';') | Some('|') | Some('&') | Some(')') | Some('}') | Some('\n')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        let mut p = Parser::new("echo hello world");
        let list = p.parse_list().unwrap();
        assert_eq!(list.andors.len(), 1);
        match &list.andors[0].pipelines[0].commands[0] {
            Command::Simple(cmd) => {
                assert_eq!(cmd.words.len(), 3);
                assert_eq!(cmd.words[0].text, "echo");
            }
            _ => panic!("expected simple"),
        }
    }

    #[test]
    fn parses_if() {
        let mut p = Parser::new("if true; then echo yes; fi");
        let list = p.parse_list().unwrap();
        assert_eq!(list.andors.len(), 1);
        match &list.andors[0].pipelines[0].commands[0] {
            Command::If { .. } => {}
            _ => panic!("expected if"),
        }
    }

    #[test]
    fn parse_if_cond_stops_at_then() {
        let mut p = Parser::new("if true; then echo yes; fi");
        p.expect_word("if").unwrap();
        let cond = p.parse_list_until(&["then"]).unwrap();
        assert_eq!(cond.andors.len(), 1);
        p.expect_word("then").unwrap();
    }

    #[test]
    fn parses_pipeline() {
        let mut p = Parser::new("echo hi | wc -c");
        let list = p.parse_list().unwrap();
        assert_eq!(list.andors[0].pipelines[0].commands.len(), 2);
    }

    #[test]
    fn parses_if_elif_else() {
        let input = "if false; then echo a; elif true; then echo b; else echo c; fi";
        let mut p = Parser::new(input);
        let list = p.parse_list().unwrap();
        match &list.andors[0].pipelines[0].commands[0] {
            Command::If {
                elifs, else_part, ..
            } => {
                assert_eq!(elifs.len(), 1);
                assert!(else_part.is_some());
            }
            other => panic!("expected if, got {other:?}"),
        }
    }

    #[test]
    fn parses_for_empty_in_list() {
        let mut p = Parser::new("for x in; do echo $x; done");
        let list = p.parse_list().unwrap();
        match &list.andors[0].pipelines[0].commands[0] {
            Command::For { items, has_in, .. } => {
                assert!(*has_in);
                assert!(items.is_empty());
            }
            other => panic!("expected for, got {other:?}"),
        }
    }

    #[test]
    fn parses_for_empty_in_list_with_space_before_semi() {
        let mut p = Parser::new("for x in ; do echo $x; done");
        let list = p.parse_list().unwrap();
        match &list.andors[0].pipelines[0].commands[0] {
            Command::For { items, has_in, .. } => {
                assert!(*has_in);
                assert!(items.is_empty());
            }
            other => panic!("expected for, got {other:?}"),
        }
    }

    #[test]
    fn parses_for_without_in() {
        let mut p = Parser::new("for x; do echo $x; done");
        let list = p.parse_list().unwrap();
        match &list.andors[0].pipelines[0].commands[0] {
            Command::For { has_in, .. } => assert!(!*has_in),
            other => panic!("expected for, got {other:?}"),
        }
    }

    #[test]
    fn parses_multiline_while() {
        let input = "while [ 0 -lt 2 ]; do\ni=1\ndone";
        let mut p = Parser::new(input);
        match p.parse_command() {
            Ok(Command::While { .. }) => {}
            Err(e) => panic!("{e:?}"),
            Ok(other) => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parse_utf8_word_with_redirect() {
        let mut p = Parser::new("idޚcho>&k; ");
        let list = p.parse_list().unwrap();
        assert_eq!(list.andors.len(), 1);
    }

    #[test]
    fn parses_function_def() {
        let input = "f() { echo hi; }; f";
        let mut p = Parser::new(input);
        let list = p.parse_list().unwrap();
        match &list.andors[0].pipelines[0].commands[0] {
            Command::FunctionDef { name, .. } => assert_eq!(name, "f"),
            other => panic!("expected function def, got {other:?}"),
        }
    }

    #[test]
    fn parses_multiline_function_def() {
        let input = "f() {\n  echo hi\n}\n";
        let mut p = Parser::new(input);
        let list = p.parse_list().unwrap();
        match &list.andors[0].pipelines[0].commands[0] {
            Command::FunctionDef { name, body } => {
                assert_eq!(name, "f");
                assert_eq!(body.andors.len(), 1);
            }
            other => panic!("expected function def, got {other:?}"),
        }
    }

    #[test]
    fn parses_function_script() {
        let input = "f() { local x=inner; echo $x; return 5; }; f; echo $?";
        let mut p = Parser::new(input);
        let list = p.parse_list().unwrap();
        assert_eq!(list.andors.len(), 3);
        match &list.andors[1].pipelines[0].commands[0] {
            Command::Simple(cmd) => assert_eq!(cmd.words[0].text, "f"),
            other => panic!("expected call f, got {other:?}"),
        }
    }

    #[test]
    fn deep_braces_return_syntax_error() {
        let input = "{".repeat(MAX_PARSE_DEPTH + 1);
        let mut p = Parser::new(&input);
        assert!(matches!(p.parse_list(), Err(ParseError::Syntax)));
    }
}
