use crate::sys;
use crate::usage;
use rustix::fs::{Access, FileType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestError {
    Syntax,
}

pub type TestResult<T> = Result<T, TestError>;

const SYNTAX: TestError = TestError::Syntax;

pub fn run(args: &[&str]) -> i32 {
    let args = normalize_bracket(args);
    match eval(args) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(TestError::Syntax) => 2,
    }
}

pub fn eval_args(args: &[&str]) -> TestResult<bool> {
    eval(args)
}

pub fn normalize_bracket<'a>(args: &'a [&'a str]) -> &'a [&'a str] {
    if args.last() == Some(&"]") {
        &args[..args.len() - 1]
    } else {
        args
    }
}

fn eval(args: &[&str]) -> TestResult<bool> {
    let mut parser = Parser::new(args);
    let value = parser.or_expr()?;
    if !parser.at_end() {
        return Err(SYNTAX);
    }
    Ok(value)
}

struct Parser<'a> {
    args: &'a [&'a str],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(args: &'a [&'a str]) -> Self {
        Self { args, pos: 0 }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.args.len()
    }

    fn peek(&self) -> Option<&'a str> {
        self.args.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<&'a str> {
        if self.at_end() {
            None
        } else {
            let value = self.args[self.pos];
            self.pos += 1;
            Some(value)
        }
    }

    fn or_expr(&mut self) -> TestResult<bool> {
        let mut value = self.and_expr()?;
        while self.peek() == Some("-o") {
            self.next();
            value = self.and_expr()? || value;
        }
        Ok(value)
    }

    fn and_expr(&mut self) -> TestResult<bool> {
        let mut value = self.not_expr()?;
        while self.peek() == Some("-a") {
            self.next();
            value = self.not_expr()? && value;
        }
        Ok(value)
    }

    fn not_expr(&mut self) -> TestResult<bool> {
        if self.peek() == Some("!") {
            self.next();
            return Ok(!self.not_expr()?);
        }
        self.primary()
    }

    fn primary(&mut self) -> TestResult<bool> {
        if self.at_end() {
            return Err(SYNTAX);
        }

        if self.peek() == Some("(") {
            self.next();
            let value = self.or_expr()?;
            if self.next() != Some(")") {
                return Err(SYNTAX);
            }
            return Ok(value);
        }

        if let Some(op) = self.peek().and_then(unary_op) {
            self.next();
            let operand = self.next().ok_or(SYNTAX)?;
            return unary_test(op, operand);
        }

        let left = self.next().ok_or(SYNTAX)?;
        if self.at_end() {
            return Ok(!left.is_empty());
        }

        let op = self.next().ok_or(SYNTAX)?;
        let right = self.next().ok_or(SYNTAX)?;
        binary_test(left, op, right)
    }
}

#[derive(Clone, Copy)]
enum UnaryOp {
    Exists,
    File,
    Dir,
    Symlink,
    Block,
    Char,
    Fifo,
    Socket,
    Readable,
    Writable,
    Executable,
    NonEmpty,
    Empty,
    SetUid,
    SetGid,
    Sticky,
    NonZeroSize,
}

fn unary_op(s: &str) -> Option<UnaryOp> {
    match s {
        "-e" => Some(UnaryOp::Exists),
        "-f" => Some(UnaryOp::File),
        "-d" => Some(UnaryOp::Dir),
        "-h" | "-L" => Some(UnaryOp::Symlink),
        "-b" => Some(UnaryOp::Block),
        "-c" => Some(UnaryOp::Char),
        "-p" => Some(UnaryOp::Fifo),
        "-S" => Some(UnaryOp::Socket),
        "-r" => Some(UnaryOp::Readable),
        "-w" => Some(UnaryOp::Writable),
        "-x" => Some(UnaryOp::Executable),
        "-n" => Some(UnaryOp::NonEmpty),
        "-z" => Some(UnaryOp::Empty),
        "-u" => Some(UnaryOp::SetUid),
        "-g" => Some(UnaryOp::SetGid),
        "-k" => Some(UnaryOp::Sticky),
        "-s" => Some(UnaryOp::NonZeroSize),
        _ => None,
    }
}

fn unary_test(op: UnaryOp, operand: &str) -> TestResult<bool> {
    match op {
        UnaryOp::NonEmpty => Ok(!operand.is_empty()),
        UnaryOp::Empty => Ok(operand.is_empty()),
        UnaryOp::Exists => Ok(sys::exists(operand)),
        UnaryOp::Readable => Ok(sys::check_access(operand, Access::READ_OK)),
        UnaryOp::Writable => Ok(sys::check_access(operand, Access::WRITE_OK)),
        UnaryOp::Executable => Ok(sys::check_access(operand, Access::EXEC_OK)),
        _ => match sys::lstat(operand) {
            Ok(st) => {
                let ft = FileType::from_raw_mode(st.st_mode);
                let mode = st.st_mode;
                Ok(match op {
                    UnaryOp::File => ft.is_file(),
                    UnaryOp::Dir => ft.is_dir(),
                    UnaryOp::Symlink => ft.is_symlink(),
                    UnaryOp::Block => ft.is_block_device(),
                    UnaryOp::Char => ft.is_char_device(),
                    UnaryOp::Fifo => ft.is_fifo(),
                    UnaryOp::Socket => ft.is_socket(),
                    UnaryOp::SetUid => mode & 0o4000 != 0,
                    UnaryOp::SetGid => mode & 0o2000 != 0,
                    UnaryOp::Sticky => mode & 0o1000 != 0,
                    UnaryOp::NonZeroSize => st.st_size > 0,
                    _ => unreachable!(),
                })
            }
            Err(_) => Ok(false),
        },
    }
}

fn binary_test(left: &str, op: &str, right: &str) -> TestResult<bool> {
    match op {
        "=" | "==" => Ok(left == right),
        "!=" => Ok(left != right),
        "<" => Ok(left < right),
        ">" => Ok(left > right),
        "-eq" => int_compare(left, right, |a, b| a == b),
        "-ne" => int_compare(left, right, |a, b| a != b),
        "-lt" => int_compare(left, right, |a, b| a < b),
        "-le" => int_compare(left, right, |a, b| a <= b),
        "-gt" => int_compare(left, right, |a, b| a > b),
        "-ge" => int_compare(left, right, |a, b| a >= b),
        "-nt" => file_mtime_compare(left, right, |a, b| a > b),
        "-ot" => file_mtime_compare(left, right, |a, b| a < b),
        "-ef" => same_file(left, right),
        _ => {
            usage("test", &format!("unknown operator '{op}'"));
            Err(SYNTAX)
        }
    }
}

fn int_compare(left: &str, right: &str, cmp: fn(i64, i64) -> bool) -> TestResult<bool> {
    let left = left.parse::<i64>().map_err(|_| SYNTAX)?;
    let right = right.parse::<i64>().map_err(|_| SYNTAX)?;
    Ok(cmp(left, right))
}

fn file_mtime_compare(left: &str, right: &str, cmp: fn(i64, i64) -> bool) -> TestResult<bool> {
    let left = sys::stat(left).map_err(|_| SYNTAX)?.st_mtime;
    let right = sys::stat(right).map_err(|_| SYNTAX)?.st_mtime;
    Ok(cmp(left, right))
}

fn same_file(left: &str, right: &str) -> TestResult<bool> {
    let left = sys::stat(left).map_err(|_| SYNTAX)?;
    let right = sys::stat(right).map_err(|_| SYNTAX)?;
    Ok(left.st_dev == right.st_dev && left.st_ino == right.st_ino)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bracket() {
        assert_eq!(normalize_bracket(&["-f", "x", "]"]), &["-f", "x"]);
    }

    #[test]
    fn eval_non_empty_string() {
        assert!(eval(&["hello"]).unwrap());
        assert!(!eval(&[""]).unwrap());
    }

    #[test]
    fn eval_string_equal() {
        assert!(eval(&["a", "=", "a"]).unwrap());
        assert!(!eval(&["a", "=", "b"]).unwrap());
    }

    #[test]
    fn eval_integer_compare() {
        assert!(eval(&["2", "-gt", "1"]).unwrap());
        assert!(!eval(&["1", "-gt", "2"]).unwrap());
    }
}
