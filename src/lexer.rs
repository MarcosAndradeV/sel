use std::iter::Peekable;
use std::str::Chars;

use crate::diagnostics::*;

type Result<T> = std::result::Result<T, SelError>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loc {
    pub line: usize,
    pub col: usize,
}

impl Default for Loc {
    fn default() -> Self {
        Self { line: 1, col: 1 }
    }
}

impl std::fmt::Display for Loc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// The numerical base of a parsed number token (e.g., Binary, Octal, Decimal, Hexadecimal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberBase {
    B,
    O,
    D,
    X,
}

impl NumberBase {
    pub fn radix(&self) -> u32 {
        match self {
            NumberBase::B => 2,
            NumberBase::O => 8,
            NumberBase::D => 10,
            NumberBase::X => 16,
        }
    }
}

impl From<u32> for NumberBase {
    fn from(value: u32) -> Self {
        match value {
            2 => Self::B,
            8 => Self::O,
            10 => Self::D,
            16 => Self::X,
            _ => panic!("Unknown base"),
        }
    }
}

impl From<NumberBase> for u32 {
    fn from(val: NumberBase) -> Self {
        match val {
            NumberBase::B => 2,
            NumberBase::O => 8,
            NumberBase::D => 10,
            NumberBase::X => 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    OpenParen,
    CloseParen,
    Quote,
    QuasiQuote,
    Unquote,
    UnquoteSplicing,
    String,
    Identifier,
    Number(NumberBase),
    Boolean,
    Ampersand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub source: String,
    pub loc: Loc,
}

pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            line: 1,
            col: 1,
        }
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else if c == ';' {
                while let Some(ch) = self.advance() {
                    if ch == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Option<Token>> {
        self.skip_whitespace_and_comments();

        let start_loc = Loc {
            line: self.line,
            col: self.col,
        };
        let Some(&c) = self.peek() else {
            return Ok(None);
        };

        match c {
            '(' => {
                self.advance();
                Ok(Some(Token {
                    kind: TokenKind::OpenParen,
                    source: "(".into(),
                    loc: start_loc,
                }))
            }
            ')' => {
                self.advance();
                Ok(Some(Token {
                    kind: TokenKind::CloseParen,
                    source: ")".into(),
                    loc: start_loc,
                }))
            }
            '&' => {
                self.advance();
                Ok(Some(Token {
                    kind: TokenKind::Ampersand,
                    source: "&".into(),
                    loc: start_loc,
                }))
            }
            '\'' => {
                self.advance();
                Ok(Some(Token {
                    kind: TokenKind::Quote,
                    source: "'".into(),
                    loc: start_loc,
                }))
            }
            '`' => {
                self.advance();
                Ok(Some(Token {
                    kind: TokenKind::QuasiQuote,
                    source: "`".into(),
                    loc: start_loc,
                }))
            }
            '~' => {
                self.advance();
                if let Some(&'@') = self.peek() {
                    self.advance();
                    Ok(Some(Token {
                        kind: TokenKind::UnquoteSplicing,
                        source: "~@".into(),
                        loc: start_loc,
                    }))
                } else {
                    Ok(Some(Token {
                        kind: TokenKind::Unquote,
                        source: "~".into(),
                        loc: start_loc,
                    }))
                }
            }
            '"' => {
                self.advance();
                let mut string = String::new();
                while let Some(&ch) = self.peek() {
                    if ch == '"' {
                        self.advance();
                        break;
                    }
                    if ch == '\\' {
                        self.advance();
                        if let Some(escaped) = self.advance() {
                            match escaped {
                                'n' => string.push('\n'),
                                't' => string.push('\t'),
                                'r' => string.push('\r'),
                                '\\' => string.push('\\'),
                                '"' => string.push('"'),
                                _ => string.push(escaped),
                            }
                        } else {
                            return Err(SelError {
                                loc: start_loc,
                                kind: SelErrorKind::UnterminatedString,
                            });
                        }
                    } else {
                        string.push(self.advance().unwrap());
                    }
                }
                Ok(Some(Token {
                    kind: TokenKind::String,
                    source: string,
                    loc: start_loc,
                }))
            }
            '#' => {
                self.advance();
                if let Some(&c2) = self.peek()
                    && (c2 == 't' || c2 == 'f') {
                        self.advance();
                        return Ok(Some(Token {
                            kind: TokenKind::Boolean,
                            source: format!("#{}", c2),
                            loc: start_loc,
                        }));
                    }
                Err(SelError {
                    loc: start_loc,
                    kind: SelErrorKind::Generic("Invalid character following #".into()),
                })
            }
            _ => {
                let mut ident = String::new();
                while let Some(&ch) = self.peek() {
                    if ch.is_whitespace() || "()\"'`,;".contains(ch) {
                        break;
                    }
                    ident.push(self.advance().unwrap());
                }
                if ident.is_empty() {
                    return Err(SelError {
                        loc: start_loc,
                        kind: SelErrorKind::UnexpectedToken(
                            self.advance().unwrap().to_string(),
                        ),
                    });
                }

                let (is_num, base) = if let Some(stripped) = ident
                    .strip_prefix("0x")
                    .or_else(|| ident.strip_prefix("0X"))
                {
                    (i64::from_str_radix(stripped, 16).is_ok(), NumberBase::X)
                } else if let Some(stripped) = ident
                    .strip_prefix("0b")
                    .or_else(|| ident.strip_prefix("0B"))
                {
                    (i64::from_str_radix(stripped, 2).is_ok(), NumberBase::B)
                } else if let Some(stripped) = ident
                    .strip_prefix("0o")
                    .or_else(|| ident.strip_prefix("0O"))
                {
                    (i64::from_str_radix(stripped, 8).is_ok(), NumberBase::O)
                } else {
                    (
                        ident.parse::<i64>().is_ok() || ident.parse::<f64>().is_ok(),
                        NumberBase::D,
                    )
                };

                if is_num {
                    Ok(Some(Token {
                        kind: TokenKind::Number(base),
                        source: ident,
                        loc: start_loc,
                    }))
                } else {
                    Ok(Some(Token {
                        kind: TokenKind::Identifier,
                        source: ident,
                        loc: start_loc,
                    }))
                }
            }
        }
    }
}
