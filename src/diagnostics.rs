use crate::lexer::Loc;
use crate::runtime::*;

#[derive(Debug, Clone)]
pub enum SelErrorKind {
    UnexpectedEOF,
    UnexpectedToken(String),
    UndefinedVariable(u32),
    ArityMismatch { expected: usize, actual: usize },
    UnboundVariable(u32),
    InvalidNumber(String),
    UnterminatedString,
    Generic(String),
}

#[derive(Debug, Clone)]
pub struct SelError {
    pub loc: Loc,
    pub kind: SelErrorKind,
}

impl std::fmt::Display for SelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            SelErrorKind::UnexpectedEOF => write!(f, "{}: Unexpected EOF", self.loc),
            SelErrorKind::UnexpectedToken(s) => {
                write!(f, "{}: Unexpected token `{}`", self.loc, s)
            }
            SelErrorKind::UndefinedVariable(id) => {
                write!(f, "{}: Undefined variable `{}`", self.loc, lookup(*id))
            }
            SelErrorKind::ArityMismatch { expected, actual } => write!(
                f,
                "{}: Arity mismatch: expected {}, got {}",
                self.loc, expected, actual
            ),
            SelErrorKind::UnboundVariable(id) => {
                write!(f, "{}: Unbound variable in set!: {}", self.loc, lookup(*id))
            }
            SelErrorKind::InvalidNumber(s) => {
                write!(f, "{}: Invalid number format `{}`", self.loc, s)
            }
            SelErrorKind::UnterminatedString => write!(f, "{}: Unterminated string", self.loc),
            SelErrorKind::Generic(s) => write!(f, "{}: {}", self.loc, s),
        }
    }
}

impl std::error::Error for SelError {}
