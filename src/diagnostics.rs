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
    Trace(String),
    SandboxViolation(Loc, String),
}

fn format_snippet(f: &mut std::fmt::Formatter<'_>, loc: Loc) -> std::fmt::Result {
    let filename = lookup(loc.file_id);
    if filename.is_empty() || filename == "<repl>" || filename == "<eval>" {
        return Ok(());
    }
    if let Ok(content) = std::fs::read_to_string(&filename) {
        let lines: Vec<&str> = content.lines().collect();
        if loc.line > 0 && (loc.line as usize) <= lines.len() {
            let line_idx = (loc.line - 1) as usize;
            let line_text = lines[line_idx];
            let col = if loc.col > 0 { (loc.col - 1) as usize } else { 0 };
            let width = if loc.byte_len > 0 { loc.byte_len as usize } else { 1 };

            write!(f, "\n\n  --> {}:{}:{}", filename, loc.line, loc.col)?;
            write!(f, "\n   |")?;
            write!(f, "\n{:4} | {}", loc.line, line_text)?;
            write!(f, "\n   | {}{}", " ".repeat(col), "^".repeat(width.max(1)))?;
        }
    }
    Ok(())
}

impl std::fmt::Display for SelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, ":- ")?;
        match &self {
            Self::UnexpectedEOF(loc) => {
                write!(
                    f,
                    "syntax error at {}:\n\nCaused by:\n    Unexpected EOF",
                    loc
                )?;
                format_snippet(f, *loc)
            }
            Self::UnexpectedToken(loc, s) => {
                write!(
                    f,
                    "syntax error at {}:\n\nCaused by:\n    Unexpected token `{}`",
                    loc, s
                )?;
                format_snippet(f, *loc)
            }
            Self::SyntaxError(loc, msg) => {
                write!(f, "syntax error at {}:\n\nCaused by:\n    {}", loc, msg)?;
                format_snippet(f, *loc)
            }
            Self::UndefinedVariable(loc, id) => {
                write!(
                    f,
                    "name error at {}:\n\nCaused by:\n    Undefined variable `{}`",
                    loc,
                    lookup(*id)
                )?;
                format_snippet(f, *loc)
            }
            Self::ArityMismatch {
                loc,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "argument error at {}:\n\nCaused by:\n    Arity mismatch: expected {}, got {}",
                    loc, expected, actual
                )?;
                format_snippet(f, *loc)
            }
            Self::UnboundVariable(loc, id) => {
                write!(
                    f,
                    "assignment error at {}:\n\nCaused by:\n    Unbound variable in set!: {}",
                    loc,
                    lookup(*id)
                )?;
                format_snippet(f, *loc)
            }
            Self::InvalidNumber(t) => {
                write!(
                    f,
                    "syntax error at {}:\n\nCaused by:\n    Invalid number format `{}`",
                    t.loc, t.source
                )?;
                format_snippet(f, t.loc)
            }
            Self::UnterminatedString(loc) => {
                write!(
                    f,
                    "syntax error at {}:\n\nCaused by:\n    Unterminated string",
                    loc
                )?;
                format_snippet(f, *loc)
            }
            Self::Runtime(loc, s) => {
                write!(f, "runtime error at {}:\n\nCaused by:\n    {}", loc, s)?;
                format_snippet(f, *loc)
            }
            Self::TypeError(loc, s) => {
                write!(f, "type error at {}:\n\nCaused by:\n    {}", loc, s)?;
                format_snippet(f, *loc)
            }
            Self::SandboxViolation(loc, s) => {
                write!(f, "sandbox error at {}:\n\nCaused by:\n    {}", loc, s)?;
                format_snippet(f, *loc)
            }
            Self::Internal(s) => write!(f, "internal error caused by:\n    {}", s),
            SelError::Trace(errs) => {
                write!(f, "{errs}")
            }
        }
    }
}

impl std::error::Error for SelError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelErrorKind {
    Syntax,
    Name,
    Arity,
    Type,
    Sandbox,
    Internal,
    Runtime,
}

impl SelError {
    pub fn kind(&self) -> SelErrorKind {
        match self {
            Self::UnexpectedEOF(_)
            | Self::UnexpectedToken(_, _)
            | Self::SyntaxError(_, _)
            | Self::InvalidNumber(_)
            | Self::UnterminatedString(_)
            | Self::Trace(_) => SelErrorKind::Syntax,
            Self::UndefinedVariable(_, _) | Self::UnboundVariable(_, _) => SelErrorKind::Name,
            Self::ArityMismatch { .. } => SelErrorKind::Arity,
            Self::TypeError(_, _) => SelErrorKind::Type,
            Self::SandboxViolation(_, _) => SelErrorKind::Sandbox,
            Self::Internal(_) => SelErrorKind::Internal,
            Self::Runtime(_, _) => SelErrorKind::Runtime,
        }
    }

    pub fn loc(&self) -> Option<Loc> {
        match self {
            Self::UnexpectedEOF(loc)
            | Self::UnexpectedToken(loc, _)
            | Self::SyntaxError(loc, _)
            | Self::UndefinedVariable(loc, _)
            | Self::UnboundVariable(loc, _)
            | Self::UnterminatedString(loc)
            | Self::Runtime(loc, _)
            | Self::TypeError(loc, _)
            | Self::SandboxViolation(loc, _) => Some(*loc),
            Self::ArityMismatch { loc, .. } => Some(*loc),
            Self::InvalidNumber(token) => Some(token.loc),
            Self::Internal(_) | Self::Trace(_) => None,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::UnexpectedEOF(_) => "Unexpected EOF".to_string(),
            Self::UnexpectedToken(_, s) => format!("Unexpected token `{}`", s),
            Self::SyntaxError(_, msg) => msg.clone(),
            Self::UndefinedVariable(_, id) => format!("Undefined variable `{}`", lookup(*id)),
            Self::UnboundVariable(_, id) => format!("Unbound variable in set!: {}", lookup(*id)),
            Self::ArityMismatch {
                expected, actual, ..
            } => {
                format!("Arity mismatch: expected {}, got {}", expected, actual)
            }
            Self::InvalidNumber(token) => format!("Invalid number format `{}`", token.source),
            Self::UnterminatedString(_) => "Unterminated string".to_string(),
            Self::Runtime(_, msg) => msg.clone(),
            Self::TypeError(_, msg) => msg.clone(),
            Self::SandboxViolation(_, msg) => msg.clone(),
            Self::Internal(msg) => msg.clone(),
            Self::Trace(msg) => msg.clone(),
        }
    }
}
