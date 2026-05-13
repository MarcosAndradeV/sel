use crate::lexer::{Loc, Token};
use crate::types::lookup;

#[derive(Debug, Clone)]
pub enum SelError {
    UnexpectedEOF(Loc),
    UnexpectedToken(Loc, String),
    SyntaxError(Loc, String),
    UndefinedVariable(Loc, u32),
    ArityMismatch {
        loc: Loc,
        expected: usize,
        actual: usize,
    },
    UnboundVariable(Loc, u32),
    InvalidNumber(Token),
    UnterminatedString(Loc),
    Internal(String),
    Runtime(Loc, String),
    TypeError(Loc, String),
}

impl SelError {
    pub fn with_loc(self, loc: Loc) -> SelError {
        match self {
            SelError::UnexpectedEOF(_) => SelError::UnexpectedEOF(loc),
            SelError::UnexpectedToken(_, e) => SelError::UnexpectedToken(loc, e),
            SelError::SyntaxError(_, e) => SelError::SyntaxError(loc, e),
            SelError::UndefinedVariable(_, e) => SelError::UndefinedVariable(loc, e),
            SelError::ArityMismatch {
                loc: _,
                expected,
                actual,
            } => SelError::ArityMismatch {
                loc,
                expected,
                actual,
            },
            SelError::UnboundVariable(_, e) => SelError::UnboundVariable(loc, e),
            SelError::InvalidNumber(token) => SelError::InvalidNumber(token),
            SelError::UnterminatedString(_) => SelError::UnterminatedString(loc),
            SelError::Internal(e) => SelError::Internal(e),
            SelError::Runtime(_, e) => SelError::Runtime(loc, e),
            SelError::TypeError(_, e) => SelError::TypeError(loc, e),
        }
    }
}

impl std::fmt::Display for SelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::UnexpectedEOF(loc) => write!(
                f,
                "syntax error at {}:\n\nCaused by:\n    Unexpected EOF",
                loc
            ),
            Self::UnexpectedToken(loc, s) => {
                write!(
                    f,
                    "syntax error at {}:\n\nCaused by:\n    Unexpected token `{}`",
                    loc, s
                )
            }
            Self::SyntaxError(loc, msg) => {
                write!(f, "syntax error at {}:\n\nCaused by:\n    {}", loc, msg)
            }
            Self::UndefinedVariable(loc, id) => {
                write!(
                    f,
                    "name error at {}:\n\nCaused by:\n    Undefined variable `{}`",
                    loc,
                    lookup(*id)
                )
            }
            Self::ArityMismatch {
                loc,
                expected,
                actual,
            } => write!(
                f,
                "argument error at {}:\n\nCaused by:\n    Arity mismatch: expected {}, got {}",
                loc, expected, actual
            ),
            Self::UnboundVariable(loc, id) => {
                write!(
                    f,
                    "assignment error at {}:\n\nCaused by:\n    Unbound variable in set!: {}",
                    loc,
                    lookup(*id)
                )
            }
            Self::InvalidNumber(t) => {
                write!(
                    f,
                    "syntax error at {}:\n\nCaused by:\n    Invalid number format `{}`",
                    t.loc, t.source
                )
            }
            Self::UnterminatedString(loc) => write!(
                f,
                "syntax error at {}:\n\nCaused by:\n    Unterminated string",
                loc
            ),
            Self::Runtime(loc, s) => write!(f, "runtime error at {}:\n\nCaused by:\n    {}", loc, s),
            Self::TypeError(loc, s) => write!(f, "type error at {}:\n\nCaused by:\n    {}", loc, s),
            Self::Internal(s) => write!(f, "internal error caused by:\n    {}", s),
        }
    }
}

impl std::error::Error for SelError {}
