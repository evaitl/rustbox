use std::collections::HashMap;

pub fn eval(expr: &str, vars: &HashMap<String, String>) -> Result<i64, ()> {
    let tokens = tokenize(expr)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        vars,
    };
    let value = parser.expr()?;
    if parser.pos != parser.tokens.len() {
        return Err(());
    }
    Ok(value)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    vars: &'a HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Num(i64),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
}

fn tokenize(expr: &str) -> Result<Vec<Token>, ()> {
    let mut tokens = Vec::new();
    let bytes = expr.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        match b {
            b'+' => tokens.push(Token::Plus),
            b'-' => tokens.push(Token::Minus),
            b'*' => tokens.push(Token::Star),
            b'/' => tokens.push(Token::Slash),
            b'%' => tokens.push(Token::Percent),
            b'(' => tokens.push(Token::LParen),
            b')' => tokens.push(Token::RParen),
            b'0'..=b'9' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let num: i64 = expr[start..i].parse().map_err(|_| ())?;
                tokens.push(Token::Num(num));
                continue;
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    if c.is_ascii_alphanumeric() || c == b'_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Name(expr[start..i].to_string()));
                continue;
            }
            _ => return Err(()),
        }
        i += 1;
    }
    Ok(tokens)
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expr(&mut self) -> Result<i64, ()> {
        self.add_sub()
    }

    fn add_sub(&mut self) -> Result<i64, ()> {
        let mut value = self.mul_div()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.bump();
                    value = value.checked_add(self.mul_div()?).ok_or(())?;
                }
                Some(Token::Minus) => {
                    self.bump();
                    value = value.checked_sub(self.mul_div()?).ok_or(())?;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn mul_div(&mut self) -> Result<i64, ()> {
        let mut value = self.unary()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.bump();
                    value = value.checked_mul(self.unary()?).ok_or(())?;
                }
                Some(Token::Slash) => {
                    self.bump();
                    let rhs = self.unary()?;
                    if rhs == 0 {
                        return Err(());
                    }
                    value = value.checked_div(rhs).ok_or(())?;
                }
                Some(Token::Percent) => {
                    self.bump();
                    let rhs = self.unary()?;
                    if rhs == 0 {
                        return Err(());
                    }
                    value = value.checked_rem(rhs).ok_or(())?;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn unary(&mut self) -> Result<i64, ()> {
        match self.peek() {
            Some(Token::Plus) => {
                self.bump();
                self.unary()
            }
            Some(Token::Minus) => {
                self.bump();
                self.unary().and_then(|v| v.checked_neg().ok_or(()))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<i64, ()> {
        match self.bump() {
            Some(Token::Num(n)) => Ok(n),
            Some(Token::Name(name)) => {
                let value = self
                    .vars
                    .get(&name)
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                Ok(value)
            }
            Some(Token::LParen) => {
                let value = self.expr()?;
                if self.bump() != Some(Token::RParen) {
                    return Err(());
                }
                Ok(value)
            }
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_addition() {
        let vars = HashMap::new();
        assert_eq!(eval("1+2*3", &vars).unwrap(), 7);
    }

    #[test]
    fn eval_variable() {
        let mut vars = HashMap::new();
        vars.insert("i".to_string(), "4".to_string());
        assert_eq!(eval("i+1", &vars).unwrap(), 5);
    }

    #[test]
    fn eval_modulo() {
        let vars = HashMap::new();
        assert_eq!(eval("10 % 3", &vars).unwrap(), 1);
    }

    #[test]
    fn eval_divide_by_zero_fails() {
        let vars = HashMap::new();
        assert!(eval("1 / 0", &vars).is_err());
    }

    #[test]
    fn eval_unary_minus_and_parens() {
        let vars = HashMap::new();
        assert_eq!(eval("-(2 + 3)", &vars).unwrap(), -5);
    }
}
