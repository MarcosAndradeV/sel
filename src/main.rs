use anyhow::Result as AnyhowResult;
use rustyline::error::ReadlineError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::iter::Peekable;
use std::rc::Rc;
use std::str::Chars;
use std::{env, fs};

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
            SelErrorKind::UnexpectedToken(s) => write!(f, "{}: Unexpected token `{}`", self.loc, s),
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

pub type Result<T> = std::result::Result<T, SelError>;

thread_local! {
    static SYMBOLS: RefCell<SymbolTable> = RefCell::new(SymbolTable::default());
}

#[derive(Default)]
struct SymbolTable {
    map: HashMap<String, u32>,
    vec: Vec<String>,
}

impl SymbolTable {
    fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.map.get(name) {
            id
        } else {
            let id = self.vec.len() as u32;
            self.map.insert(name.to_string(), id);
            self.vec.push(name.to_string());
            id
        }
    }

    fn lookup(&self, id: u32) -> String {
        self.vec
            .get(id as usize)
            .cloned()
            .unwrap_or_else(|| format!("id:{}", id))
    }
}

pub fn intern(name: &str) -> u32 {
    SYMBOLS.with(|s| s.borrow_mut().intern(name))
}

pub fn lookup(id: u32) -> String {
    SYMBOLS.with(|s| s.borrow().lookup(id))
}

#[derive(Debug, Default)]
pub struct Env {
    bindings: HashMap<u32, Value>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Self {
            bindings: HashMap::new(),
            parent,
        }
    }

    fn get(&self, id: u32) -> Option<Value> {
        if let Some(val) = self.bindings.get(&id) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().get(id)
        } else {
            None
        }
    }

    fn insert(&mut self, id: u32, val: Value) {
        self.bindings.insert(id, val);
    }

    fn set(&mut self, id: u32, val: Value) -> bool {
        if self.bindings.contains_key(&id) {
            self.bindings.insert(id, val);
            true
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().set(id, val)
        } else {
            false
        }
    }
}

fn main() -> AnyhowResult<()> {
    let env = Rc::new(RefCell::new(Env::default()));
    sel_core::load(env.clone());

    // Load core library if exists
    if let Ok(core_src) = fs::read_to_string("core.scm") {
        match read_all(&core_src) {
            Ok(asts) => {
                if let Err(e) = execute_asts(asts, env.clone()) {
                    eprintln!("Error loading core.scm: {}", e);
                }
            }
            Err(e) => eprintln!("Error parsing core.scm: {}", e),
        }
    }

    let mut args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let script_path = args.remove(1);

        let scheme_args = args.into_iter().skip(1).map(Value::String).collect();
        env.borrow_mut()
            .insert(intern("*args*"), Value::List(scheme_args));

        let mut src = fs::read_to_string(script_path)?;
        if src.starts_with("#!") {
            if let Some(newline_idx) = src.find('\n') {
                src = src[newline_idx + 1..].to_string();
            } else {
                src = String::new();
            }
        }

        let asts = read_all(&src)?;
        execute_asts(asts, env)
            .map_err(|e| anyhow::anyhow!("{}", e))
            .map(|_| ())
    } else {
        env.borrow_mut()
            .insert(intern("*args*"), Value::List(vec![]));
        println!("Welcome to the Sel Scheme repl. (Use `quit` to exit)");
        repl("sel> ", env)
    }
}

fn repl(prompt: &str, env: Rc<RefCell<Env>>) -> AnyhowResult<()> {
    let sel_path = env::home_dir().unwrap_or_default().join(".sel");
    fs::create_dir_all(&sel_path)?;
    let hist_path = sel_path.join("history");
    let mut rl = rustyline::DefaultEditor::new()?;
    if rl.load_history(&hist_path).is_err() {
        println!("No previous history.");
    }
    loop {
        match rl.readline(prompt) {
            Ok(line) => {
                let line = line.trim();
                rl.add_history_entry(line)?;
                match line {
                    "" => continue,
                    "quit" => break,
                    _ => (),
                }

                let asts = match read_all(line) {
                    Ok(asts) => asts,
                    Err(e) => {
                        println!("Error: {e}");
                        continue;
                    }
                };
                let val = match execute_asts(asts, env.clone()) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("Error: {e}");
                        continue;
                    }
                };
                if let Err(e) = print(val) {
                    println!("Error: {e}");
                    continue;
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history(&hist_path)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Loc {
    pub line: usize,
    pub col: usize,
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

impl Token {
    pub fn source(&self) -> &str {
        &self.source
    }
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
                                kind: SelErrorKind::Generic("Unterminated string escape".into()),
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
                if let Some(&c2) = self.peek() {
                    if c2 == 't' || c2 == 'f' {
                        self.advance();
                        return Ok(Some(Token {
                            kind: TokenKind::Boolean,
                            source: format!("#{}", c2),
                            loc: start_loc,
                        }));
                    }
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
                        kind: SelErrorKind::UnexpectedToken(self.advance().unwrap().to_string()),
                    });
                }

                let (is_num, base) = if let Some(stripped) = ident.strip_prefix("0x").or_else(|| ident.strip_prefix("0X")) {
                    (i64::from_str_radix(stripped, 16).is_ok(), NumberBase::X)
                } else if let Some(stripped) = ident.strip_prefix("0b").or_else(|| ident.strip_prefix("0B")) {
                    (i64::from_str_radix(stripped, 2).is_ok(), NumberBase::B)
                } else if let Some(stripped) = ident.strip_prefix("0o").or_else(|| ident.strip_prefix("0O")) {
                    (i64::from_str_radix(stripped, 8).is_ok(), NumberBase::O)
                } else {
                    (ident.parse::<i64>().is_ok() || ident.parse::<f64>().is_ok(), NumberBase::D)
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

#[allow(dead_code)]
// DO NOT DELETE
fn read(line: &str) -> Result<Ast> {
    let mut lex = Lexer::new(line);
    let mut tokens = Vec::new();
    while let Some(t) = lex.next_token()? {
        tokens.push(t);
    }
    let mut pos = 0;
    parse_expr(&tokens, &mut pos)
}

fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<Ast> {
    if *pos >= tokens.len() {
        return Err(SelError {
            loc: Loc::default(),
            kind: SelErrorKind::UnexpectedEOF,
        });
    }
    let t = &tokens[*pos];
    *pos += 1;

    match t.kind {
        TokenKind::OpenParen => parse_list(tokens, pos, t),
        TokenKind::CloseParen => Err(SelError {
            loc: t.loc,
            kind: SelErrorKind::UnexpectedToken(")".into()),
        }),
        TokenKind::Quote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::Quote(t.loc.clone(), Box::new(expr)))
        }
        TokenKind::Ampersand => {
            if let Ast::Symbol(_, id) = parse_expr(tokens, pos)? {
                return Ok(Ast::Bind(id));
            }
            Err(SelError {
                loc: t.loc,
                kind: SelErrorKind::Generic("Expected identifier after &".into()),
            })
        }
        TokenKind::QuasiQuote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::Quasiquote(t.loc.clone(), Box::new(expr)))
        }
        TokenKind::Unquote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::Unquote(t.loc.clone(), Box::new(expr)))
        }
        TokenKind::UnquoteSplicing => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::UnquoteSplicing(t.loc.clone(), Box::new(expr)))
        }
        TokenKind::String => Ok(Ast::String(t.source.clone())),
        TokenKind::Boolean => Ok(Ast::Boolean(t.source == "#t")),
        TokenKind::Number(base) => {
            let s = match base {
                NumberBase::X => t.source.trim_start_matches("0x").trim_start_matches("0X"),
                NumberBase::B => t.source.trim_start_matches("0b").trim_start_matches("0B"),
                NumberBase::O => t.source.trim_start_matches("0o").trim_start_matches("0O"),
                NumberBase::D => &t.source,
            };
            
            if let Ok(i) = i64::from_str_radix(s, base.radix()) {
                Ok(Ast::Integer(i))
            } else if base == NumberBase::D {
                if let Ok(f) = t.source.parse::<f64>() {
                    Ok(Ast::Float(f))
                } else {
                    Err(SelError {
                        loc: t.loc.clone(),
                        kind: SelErrorKind::InvalidNumber(t.source.clone()),
                    })
                }
            } else {
                Err(SelError {
                    loc: t.loc.clone(),
                    kind: SelErrorKind::InvalidNumber(t.source.clone()),
                })
            }
        }
        TokenKind::Identifier => match t.source.as_str() {
            "nil" => Ok(Ast::Nil),
            _ => Ok(Ast::Symbol(t.loc, intern(&t.source))),
        },
    }
}

fn parse_list(tokens: &[Token], pos: &mut usize, open_token: &Token) -> Result<Ast> {
    let mut list = Vec::new();
    while *pos < tokens.len() && tokens[*pos].kind != TokenKind::CloseParen {
        list.push(parse_expr(tokens, pos)?);
    }
    if *pos >= tokens.len() {
        return Err(SelError {
            loc: open_token.loc.clone(),
            kind: SelErrorKind::Generic("Missing closing parenthesis".into()),
        });
    }
    *pos += 1; // consume ')'
    optimize_ast(list, open_token.loc.clone())
}

fn optimize_ast(list: Vec<Ast>, _loc: Loc) -> Result<Ast> {
    if list.is_empty() {
        return Ok(Ast::Nil);
    }

    if let Some(Ast::Symbol(s_loc, id)) = list.first().cloned() {
        match lookup(id).as_str() {
            "if" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Missing condition in if".into()),
                })?;
                let true_branch = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Missing true branch in if".into()),
                })?;
                let false_branch = iter.next();
                Ok(Ast::If(
                    s_loc,
                    Box::new(cond),
                    Box::new(true_branch),
                    false_branch.map(Box::new),
                ))
            }
            "lambda" => {
                let mut iter = list.into_iter().skip(1);
                let params_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Missing parameters in lambda".into()),
                })?;
                let mut params = Vec::new();
                match params_ast {
                    Ast::List(p) => {
                        for param in p {
                            if let Ast::Symbol(_, id) = param {
                                params.push(id);
                            } else if let Ast::Bind(id) = param {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            } else {
                                return Err(SelError {
                                    loc: s_loc.clone(),
                                    kind: SelErrorKind::Generic(
                                        "Expected identifier in lambda parameters".into(),
                                    ),
                                });
                            }
                        }
                    }
                    Ast::Nil => {}
                    _ => {
                        return Err(SelError {
                            loc: s_loc.clone(),
                            kind: SelErrorKind::Generic("Expected parameter list in lambda".into()),
                        });
                    }
                }
                let body = iter.collect();
                Ok(Ast::Lambda(s_loc, params, body))
            }
            "begin" => {
                let iter = list.into_iter().skip(1);
                Ok(Ast::Begin(s_loc, iter.collect()))
            }
            "define" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected identifier in define".into()),
                })?;

                if let Ast::List(mut p_list) = name_ast {
                    if p_list.is_empty() {
                        return Err(SelError {
                            loc: s_loc,
                            kind: SelErrorKind::Generic("Empty parameter list in define".into()),
                        });
                    }
                    let head = p_list.remove(0);
                    let Ast::Symbol(_, name_id) = head else {
                        return Err(SelError {
                            loc: s_loc,
                            kind: SelErrorKind::Generic(
                                "Expected identifier at head of parameter list in define".into(),
                            ),
                        });
                    };
                    let mut params = Vec::new();
                    for p in p_list {
                        match p {
                            Ast::Symbol(_, id) => params.push(id),
                            Ast::Bind(id) => {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            }
                            _ => {
                                return Err(SelError {
                                    loc: s_loc,
                                    kind: SelErrorKind::Generic(
                                        "Expected identifier in parameter list".into(),
                                    ),
                                });
                            }
                        }
                    }
                    let body: Vec<Ast> = iter.collect();
                    if body.is_empty() {
                        return Err(SelError {
                            loc: s_loc,
                            kind: SelErrorKind::Generic("Missing body in define".into()),
                        });
                    }
                    return Ok(Ast::Define(
                        s_loc.clone(),
                        name_id,
                        Box::new(Ast::Lambda(s_loc, params, body)),
                    ));
                }

                let value_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected expression in define".into()),
                })?;
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError {
                        loc: s_loc.clone(),
                        kind: SelErrorKind::Generic("Expected identifier in define".into()),
                    });
                };
                Ok(Ast::Define(s_loc, name_id, Box::new(value_ast)))
            }
            "defmacro" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected identifier in defmacro".into()),
                })?;
                let params_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected parameters in defmacro".into()),
                })?;

                let mut params = Vec::new();
                match params_ast {
                    Ast::List(p) => {
                        for param in p {
                            if let Ast::Symbol(_, id) = param {
                                params.push(id);
                            } else if let Ast::Bind(id) = param {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            } else {
                                return Err(SelError {
                                    loc: s_loc.clone(),
                                    kind: SelErrorKind::Generic(
                                        "Expected identifier in defmacro parameters".into(),
                                    ),
                                });
                            }
                        }
                    }
                    Ast::Nil => {}
                    _ => {
                        return Err(SelError {
                            loc: s_loc.clone(),
                            kind: SelErrorKind::Generic(
                                "Expected parameter list in defmacro".into(),
                            ),
                        });
                    }
                }
                let body: Vec<Ast> = iter.collect();
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError {
                        loc: s_loc.clone(),
                        kind: SelErrorKind::Generic("Expected identifier in defmacro".into()),
                    });
                };
                Ok(Ast::DefMacro(
                    s_loc,
                    name_id,
                    Box::new(Ast::Lambda(s_loc.clone(), params, body)),
                ))
            }
            "set!" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected identifier in set!".into()),
                })?;
                let value_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected expression in set!".into()),
                })?;
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError {
                        loc: s_loc.clone(),
                        kind: SelErrorKind::Generic("Expected identifier in set!".into()),
                    });
                };
                Ok(Ast::Set(s_loc, name_id, Box::new(value_ast)))
            }
            "let" => {
                let mut iter = list.into_iter().skip(1);
                let bindings_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected bindings in let".into()),
                })?;
                let mut bindings = Vec::new();
                match bindings_ast {
                    Ast::List(b) => {
                        for bind in b {
                            if let Ast::List(mut pair) = bind {
                                if pair.len() != 2 {
                                    return Err(SelError {
                                        loc: s_loc.clone(),
                                        kind: SelErrorKind::Generic(
                                            "Invalid binding pair in let".into(),
                                        ),
                                    });
                                }
                                let val = pair.pop().unwrap();
                                let name = pair.pop().unwrap();
                                if let Ast::Symbol(_, name_id) = name {
                                    bindings.push((name_id, val));
                                } else {
                                    return Err(SelError {
                                        loc: s_loc.clone(),
                                        kind: SelErrorKind::Generic(
                                            "Expected identifier in let binding".into(),
                                        ),
                                    });
                                }
                            } else {
                                return Err(SelError {
                                    loc: s_loc.clone(),
                                    kind: SelErrorKind::Generic(
                                        "Expected binding pair in let".into(),
                                    ),
                                });
                            }
                        }
                    }
                    Ast::Nil => {}
                    _ => {
                        return Err(SelError {
                            loc: s_loc.clone(),
                            kind: SelErrorKind::Generic("Expected binding list in let".into()),
                        });
                    }
                }
                let body = iter.collect();
                Ok(Ast::Let(s_loc, bindings, body))
            }
            "quote" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected expression in quote".into()),
                })?;
                Ok(Ast::Quote(s_loc, Box::new(expr)))
            }
            "quasiquote" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().ok_or_else(|| SelError {
                    loc: s_loc.clone(),
                    kind: SelErrorKind::Generic("Expected expression in quasiquote".into()),
                })?;
                Ok(Ast::Quasiquote(s_loc, Box::new(expr)))
            }
            "and" => {
                let iter = list.into_iter().skip(1);
                Ok(Ast::And(s_loc, iter.collect()))
            }
            "or" => {
                let iter = list.into_iter().skip(1);
                Ok(Ast::Or(s_loc, iter.collect()))
            }
            _ => Ok(Ast::List(list)),
        }
    } else {
        Ok(Ast::List(list))
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Symbol(u32),
    List(Vec<Value>),
    NativeFunction(fn(Vec<Value>, Rc<RefCell<Env>>) -> Result<Value>),
    Closure {
        params: Vec<u32>,
        chunk: Rc<Chunk>,
        env: Rc<RefCell<Env>>,
    },
    Macro {
        params: Vec<u32>,
        chunk: Rc<Chunk>,
        env: Rc<RefCell<Env>>,
    },
    Pointer(usize),
    Library(Rc<libloading::Library>),
}

impl Value {
    pub fn display(&self) -> String {
        display_value(self)
    }
}

fn format_value_internal(val: &Value, display: bool) -> String {
    match val {
        Value::Nil => "()".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => {
            if display {
                s.clone()
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Boolean(b) => {
            if *b {
                "#t".to_string()
            } else {
                "#f".to_string()
            }
        }
        Value::Symbol(id) => lookup(*id),
        Value::List(l) => {
            let mut s = String::from("(");
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(&format_value_internal(v, false));
            }
            s.push(')');
            s
        }
        Value::NativeFunction(_) => "<native function>".to_string(),
        Value::Closure { .. } => "<closure>".to_string(),
        Value::Macro { .. } => "<macro>".to_string(),
        Value::Pointer(p) => format!("<pointer: {:#x}>", p),
        Value::Library(_) => "<library>".to_string(),
    }
}

fn format_value(val: &Value) -> String {
    format_value_internal(val, false)
}

fn display_value(val: &Value) -> String {
    format_value_internal(val, true)
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_value(self))
    }
}

#[derive(Debug, Clone)]

pub enum Ast {
    Define(Loc, u32, Box<Ast>),
    DefMacro(Loc, u32, Box<Ast>),
    Let(Loc, Vec<(u32, Ast)>, Vec<Ast>),
    Set(Loc, u32, Box<Ast>),
    If(Loc, Box<Ast>, Box<Ast>, Option<Box<Ast>>),
    Lambda(Loc, Vec<u32>, Vec<Ast>),
    Begin(Loc, Vec<Ast>),
    Quote(Loc, Box<Ast>),
    Quasiquote(Loc, Box<Ast>),
    Unquote(Loc, Box<Ast>),
    UnquoteSplicing(Loc, Box<Ast>),
    And(Loc, Vec<Ast>),
    Or(Loc, Vec<Ast>),
    Bind(u32),
    Nil,
    Symbol(Loc, u32),
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    List(Vec<Self>),
}

impl std::fmt::Display for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ast::Define(..) => write!(f, "define"),
            Ast::DefMacro(..) => write!(f, "defmacro"),
            Ast::Let(..) => write!(f, "let"),
            Ast::Set(..) => write!(f, "set"),
            Ast::If(..) => write!(f, "if"),
            Ast::Lambda(..) => write!(f, "lambda"),
            Ast::Begin(..) => write!(f, "begin"),
            Ast::Quote(..) => write!(f, "quote"),
            Ast::Quasiquote(..) => write!(f, "quasiquote"),
            Ast::Unquote(..) => write!(f, "unquote"),
            Ast::UnquoteSplicing(..) => write!(f, "unquote-splicing"),
            Ast::And(..) => write!(f, "and"),
            Ast::Or(..) => write!(f, "or"),
            Ast::Nil => write!(f, "nil"),
            Ast::Symbol(_, id) => write!(f, "{}", lookup(*id)),
            Ast::Integer(i) => write!(f, "{i}"),
            Ast::Float(n) => write!(f, "{n}"),
            Ast::String(s) => write!(f, "\"{s}\""),
            Ast::Boolean(b) => write!(f, "{}", if *b { "#t" } else { "#f" }),
            Ast::List(_) => write!(f, "<list>"),
            Ast::Bind(id) => write!(f, "&{}", lookup(*id)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    Constant(usize),
    LoadVar(u32),
    StoreVar(u32),
    DefVar(u32),
    Pop,
    JumpIfFalse(usize),
    Jump(usize),
    Call(usize),
    MakeClosure(usize),
    MakeMacro(u32, usize),
    Return,
    BuildEnv(Vec<u32>),
    PopEnv,
    MakeList(usize),
    ConcatList(usize),
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
        }
    }

    pub fn write(&mut self, op: OpCode) {
        self.code.push(op);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

fn ast_to_value(ast: Ast) -> Value {
    match ast {
        Ast::Symbol(_, id) => Value::Symbol(id),
        Ast::Integer(i) => Value::Integer(i),
        Ast::Float(f) => Value::Float(f),
        Ast::String(s) => Value::String(s),
        Ast::Boolean(b) => Value::Boolean(b),
        Ast::Nil => Value::Nil,
        Ast::List(l) => Value::List(l.into_iter().map(ast_to_value).collect()),
        Ast::Define(_, id, val) => Value::List(vec![
            Value::Symbol(intern("define")),
            Value::Symbol(id),
            ast_to_value(*val),
        ]),
        Ast::DefMacro(_, id, val) => Value::List(vec![
            Value::Symbol(intern("defmacro")),
            Value::Symbol(id),
            ast_to_value(*val),
        ]),
        Ast::Set(_, id, val) => Value::List(vec![
            Value::Symbol(intern("set!")),
            Value::Symbol(id),
            ast_to_value(*val),
        ]),
        Ast::If(_, cond, t, f) => {
            let mut list = vec![
                Value::Symbol(intern("if")),
                ast_to_value(*cond),
                ast_to_value(*t),
            ];
            if let Some(f) = f {
                list.push(ast_to_value(*f));
            }
            Value::List(list)
        }
        Ast::Lambda(_, params, body) => {
            let mut list = vec![
                Value::Symbol(intern("lambda")),
                Value::List(params.into_iter().map(Value::Symbol).collect()),
            ];
            list.extend(body.into_iter().map(ast_to_value));
            Value::List(list)
        }
        Ast::Begin(_, body) => {
            let mut list = vec![Value::Symbol(intern("begin"))];
            list.extend(body.into_iter().map(ast_to_value));
            Value::List(list)
        }
        Ast::Let(_, bindings, body) => {
            let mut list = vec![Value::Symbol(intern("let"))];
            let mut bind_list = Vec::new();
            for (id, val) in bindings {
                bind_list.push(Value::List(vec![Value::Symbol(id), ast_to_value(val)]));
            }
            list.push(Value::List(bind_list));
            list.extend(body.into_iter().map(ast_to_value));
            Value::List(list)
        }
        Ast::Quote(_, val) => Value::List(vec![Value::Symbol(intern("quote")), ast_to_value(*val)]),
        Ast::Quasiquote(_, val) => Value::List(vec![
            Value::Symbol(intern("quasiquote")),
            ast_to_value(*val),
        ]),
        Ast::Unquote(_, val) => {
            Value::List(vec![Value::Symbol(intern("unquote")), ast_to_value(*val)])
        }
        Ast::UnquoteSplicing(_, val) => Value::List(vec![
            Value::Symbol(intern("unquote-splicing")),
            ast_to_value(*val),
        ]),
        Ast::And(_, exprs) => {
            let mut list = vec![Value::Symbol(intern("and"))];
            list.extend(exprs.into_iter().map(ast_to_value));
            Value::List(list)
        }
        Ast::Or(_, exprs) => {
            let mut list = vec![Value::Symbol(intern("or"))];
            list.extend(exprs.into_iter().map(ast_to_value));
            Value::List(list)
        }
        Ast::Bind(id) => Value::Symbol(id),
    }
}

fn value_to_ast(val: Value, loc: Loc) -> Result<Ast> {
    match val {
        Value::Nil => Ok(Ast::Nil),
        Value::Integer(i) => Ok(Ast::Integer(i)),
        Value::Float(f) => Ok(Ast::Float(f)),
        Value::String(s) => Ok(Ast::String(s)),
        Value::Boolean(b) => Ok(Ast::Boolean(b)),
        Value::Symbol(id) => Ok(Ast::Symbol(loc, id)),
        Value::List(l) => {
            let mut ast_list = Vec::new();
            for v in l {
                ast_list.push(value_to_ast(v, loc.clone())?);
            }
            // Need to re-run parse_list logic to get optimized AST
            // Or we could just return Ast::List and let eval handle it
            // but we want optimized AST.
            // Let's use a helper that simulates the parser's logic.
            if ast_list.is_empty() {
                return Ok(Ast::Nil);
            }
            optimize_ast(ast_list, loc)
        }
        _ => Err(SelError {
            loc,
            kind: SelErrorKind::Generic("Cannot convert function or macro to AST".into()),
        }),
    }
}

#[allow(dead_code)]
// DO NOT DELETE
fn old_bind_args(
    params: &[u32],
    args: Vec<Value>,
    parent_env: Rc<RefCell<Env>>,
    head_loc: Loc,
) -> Result<Rc<RefCell<Env>>> {
    let expected = params.len();
    let actual = args.len();
    let mut call_env = Env::new(Some(parent_env));
    let mut params_iter = params.iter();
    let mut args_iter = args.into_iter();

    let mut has_rest = false;
    while let Some(id) = params_iter.next() {
        if lookup(*id).starts_with('&') {
            let rest: Vec<Value> = args_iter.by_ref().collect();
            let name = &lookup(*id)[1..];
            call_env.insert(intern(name), Value::List(rest));
            has_rest = true;
            break;
        }
        if let Some(arg) = args_iter.next() {
            call_env.insert(*id, arg);
        } else {
            return Err(SelError {
                loc: head_loc,
                kind: SelErrorKind::ArityMismatch { expected, actual },
            });
        }
    }
    if !has_rest && args_iter.next().is_some() {
        return Err(SelError {
            loc: head_loc,
            kind: SelErrorKind::ArityMismatch { expected, actual },
        });
    }
    Ok(Rc::new(RefCell::new(call_env)))
}

pub struct Compiler<'a> {
    pub chunk: &'a mut Chunk,
}

impl<'a> Compiler<'a> {
    pub fn new(chunk: &'a mut Chunk) -> Self {
        Self { chunk }
    }

    pub fn compile(&mut self, ast: Ast) -> Result<()> {
        match ast {
            Ast::Integer(i) => {
                let idx = self.chunk.add_constant(Value::Integer(i));
                self.chunk.write(OpCode::Constant(idx));
            }
            Ast::Float(f) => {
                let idx = self.chunk.add_constant(Value::Float(f));
                self.chunk.write(OpCode::Constant(idx));
            }
            Ast::String(s) => {
                let idx = self.chunk.add_constant(Value::String(s));
                self.chunk.write(OpCode::Constant(idx));
            }
            Ast::Boolean(b) => {
                let idx = self.chunk.add_constant(Value::Boolean(b));
                self.chunk.write(OpCode::Constant(idx));
            }
            Ast::Nil => {
                let idx = self.chunk.add_constant(Value::Nil);
                self.chunk.write(OpCode::Constant(idx));
            }
            Ast::Symbol(_, id) => {
                self.chunk.write(OpCode::LoadVar(id));
            }
            Ast::Define(_, id, expr) => {
                self.compile(*expr)?;
                self.chunk.write(OpCode::DefVar(id));
            }
            Ast::Set(_, id, expr) => {
                self.compile(*expr)?;
                self.chunk.write(OpCode::StoreVar(id));
            }
            Ast::If(_, cond, true_branch, false_branch) => {
                self.compile(*cond)?;
                let jump_if_false_idx = self.chunk.code.len();
                self.chunk.write(OpCode::JumpIfFalse(0));

                self.chunk.write(OpCode::Pop);
                self.compile(*true_branch)?;

                let jump_end_idx = self.chunk.code.len();
                self.chunk.write(OpCode::Jump(0));

                self.chunk.code[jump_if_false_idx] = OpCode::JumpIfFalse(self.chunk.code.len());
                self.chunk.write(OpCode::Pop);

                if let Some(fb) = false_branch {
                    self.compile(*fb)?;
                } else {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write(OpCode::Constant(idx));
                }

                self.chunk.code[jump_end_idx] = OpCode::Jump(self.chunk.code.len());
            }
            Ast::Begin(_, mut exprs) => {
                if exprs.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write(OpCode::Constant(idx));
                } else {
                    let last = exprs.pop().unwrap();
                    for expr in exprs {
                        self.compile(expr)?;
                        self.chunk.write(OpCode::Pop);
                    }
                    self.compile(last)?;
                }
            }
            Ast::Let(_, bindings, mut body) => {
                let mut ids = Vec::new();
                for (id, val) in bindings {
                    self.compile(val)?;
                    ids.push(id);
                }
                self.chunk.write(OpCode::BuildEnv(ids));

                if body.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write(OpCode::Constant(idx));
                } else {
                    let last = body.pop().unwrap();
                    for expr in body {
                        self.compile(expr)?;
                        self.chunk.write(OpCode::Pop);
                    }
                    self.compile(last)?;
                }

                self.chunk.write(OpCode::PopEnv);
            }
            Ast::Lambda(_, params, mut body_asts) => {
                let mut child_chunk = Chunk::new();
                let mut child_compiler = Compiler::new(&mut child_chunk);

                if body_asts.is_empty() {
                    let idx = child_chunk.add_constant(Value::Nil);
                    child_chunk.write(OpCode::Constant(idx));
                } else {
                    let last = body_asts.pop().unwrap();
                    for expr in body_asts {
                        child_compiler.compile(expr)?;
                        child_compiler.chunk.write(OpCode::Pop);
                    }
                    child_compiler.compile(last)?;
                }
                child_chunk.write(OpCode::Return);

                let stub = Value::Closure {
                    params,
                    chunk: Rc::new(child_chunk),
                    env: Rc::new(RefCell::new(Env::default())),
                };
                let idx = self.chunk.add_constant(stub);
                self.chunk.write(OpCode::MakeClosure(idx));
            }
            Ast::DefMacro(_, id, expr) => {
                // Compile the macro body as a lambda, then make it a macro
                if let Ast::Lambda(_, params, mut body_asts) = *expr {
                    let mut child_chunk = Chunk::new();
                    let mut child_compiler = Compiler::new(&mut child_chunk);

                    if body_asts.is_empty() {
                        let idx = child_chunk.add_constant(Value::Nil);
                        child_chunk.write(OpCode::Constant(idx));
                    } else {
                        let last = body_asts.pop().unwrap();
                        for expr in body_asts {
                            child_compiler.compile(expr)?;
                            child_compiler.chunk.write(OpCode::Pop);
                        }
                        child_compiler.compile(last)?;
                    }
                    child_chunk.write(OpCode::Return);

                    let stub = Value::Macro {
                        params,
                        chunk: Rc::new(child_chunk),
                        env: Rc::new(RefCell::new(Env::default())),
                    };
                    let idx = self.chunk.add_constant(stub);
                    self.chunk.write(OpCode::MakeMacro(id, idx));
                } else {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic("defmacro expects a lambda".into()),
                    });
                }
            }
            Ast::List(list) => {
                if list.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write(OpCode::Constant(idx));
                    return Ok(());
                }

                let mut iter = list.into_iter();
                self.compile(iter.next().unwrap())?;

                let mut arg_count = 0;
                for arg in iter {
                    self.compile(arg)?;
                    arg_count += 1;
                }
                self.chunk.write(OpCode::Call(arg_count));
            }
            Ast::Quote(_, expr) => {
                let val = ast_to_value(*expr);
                let idx = self.chunk.add_constant(val);
                self.chunk.write(OpCode::Constant(idx));
            }
            Ast::Quasiquote(_, expr) => {
                self.compile_quasiquote(*expr)?;
            }
            Ast::And(_, exprs) => {
                if exprs.is_empty() {
                    let idx = self.chunk.add_constant(Value::Boolean(true));
                    self.chunk.write(OpCode::Constant(idx));
                    return Ok(());
                }
                let mut jump_ends = Vec::new();

                for (i, expr) in exprs.iter().enumerate() {
                    self.compile(expr.clone())?;
                    if i < exprs.len() - 1 {
                        let jmp_false = self.chunk.code.len();
                        self.chunk.write(OpCode::JumpIfFalse(0));
                        self.chunk.write(OpCode::Pop);
                        jump_ends.push(jmp_false);
                    }
                }

                let end_pos = self.chunk.code.len();
                for jmp in jump_ends {
                    self.chunk.code[jmp] = OpCode::JumpIfFalse(end_pos);
                }
            }
            Ast::Or(_, exprs) => {
                if exprs.is_empty() {
                    let idx = self.chunk.add_constant(Value::Boolean(false));
                    self.chunk.write(OpCode::Constant(idx));
                    return Ok(());
                }

                let mut jump_ends = Vec::new();
                for (i, expr) in exprs.iter().enumerate() {
                    self.compile(expr.clone())?;
                    if i < exprs.len() - 1 {
                        let jmp_false = self.chunk.code.len();
                        self.chunk.write(OpCode::JumpIfFalse(0));

                        let jmp_end = self.chunk.code.len();
                        self.chunk.write(OpCode::Jump(0));
                        jump_ends.push(jmp_end);

                        self.chunk.code[jmp_false] = OpCode::JumpIfFalse(self.chunk.code.len());
                        self.chunk.write(OpCode::Pop);
                    }
                }

                let end_pos = self.chunk.code.len();
                for jmp in jump_ends {
                    self.chunk.code[jmp] = OpCode::Jump(end_pos);
                }
            }
            Ast::Unquote(loc, _) | Ast::UnquoteSplicing(loc, _) => {
                return Err(SelError {
                    loc,
                    kind: SelErrorKind::Generic(
                        "unquote/unquote-splicing outside of quasiquote".into(),
                    ),
                });
            }
            Ast::Bind(_) => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("unexpected & binding in normal expression".into()),
                });
            }
        }
        Ok(())
    }

    fn compile_quasiquote(&mut self, ast: Ast) -> Result<()> {
        match ast {
            Ast::Unquote(_, expr) => {
                self.compile(*expr)?;
            }
            Ast::UnquoteSplicing(loc, _) => {
                return Err(SelError {
                    loc,
                    kind: SelErrorKind::Generic(
                        "unquote-splicing invalid at top level of quasiquote".into(),
                    ),
                });
            }
            Ast::List(list) => {
                let mut parts = 0;
                for item in list {
                    if let Ast::UnquoteSplicing(_, inner) = item {
                        self.compile(*inner)?;
                    } else {
                        self.compile_quasiquote(item)?;
                        self.chunk.write(OpCode::MakeList(1));
                    }
                    parts += 1;
                }
                if parts == 0 {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write(OpCode::Constant(idx));
                } else {
                    self.chunk.write(OpCode::ConcatList(parts));
                }
            }
            _ => {
                let val = ast_to_value(ast);
                let idx = self.chunk.add_constant(val);
                self.chunk.write(OpCode::Constant(idx));
            }
        }
        Ok(())
    }
}

pub struct CallFrame {
    pub chunk: Rc<Chunk>,
    pub ip: usize,
    pub env: Rc<RefCell<Env>>,
}

pub struct VM {
    pub stack: Vec<Value>,
}

impl VM {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn run(&mut self, chunk: Rc<Chunk>, env: Rc<RefCell<Env>>) -> Result<Value> {
        let mut frames = vec![CallFrame { chunk, ip: 0, env }];

        loop {
            let frame_idx = frames.len() - 1;
            let frame = &mut frames[frame_idx];

            if frame.ip >= frame.chunk.code.len() {
                // End of root chunk
                if frames.len() == 1 {
                    return Ok(self.stack.pop().unwrap_or(Value::Nil));
                } else {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic("Unexpected end of function bytecode".into()),
                    });
                }
            }

            let instruction = frame.chunk.code[frame.ip].clone();
            frame.ip += 1;

            match instruction {
                OpCode::Constant(idx) => {
                    self.stack.push(frame.chunk.constants[idx].clone());
                }
                OpCode::LoadVar(id) => {
                    if let Some(val) = frame.env.borrow().get(id) {
                        self.stack.push(val);
                    } else {
                        return Err(SelError {
                            loc: Loc::default(),
                            kind: SelErrorKind::UndefinedVariable(id),
                        });
                    }
                }
                OpCode::StoreVar(id) => {
                    let val = self.stack.last().unwrap().clone();
                    if !frame.env.borrow_mut().set(id, val) {
                        return Err(SelError {
                            loc: Loc::default(),
                            kind: SelErrorKind::UnboundVariable(id),
                        });
                    }
                }
                OpCode::DefVar(id) => {
                    let val = self.stack.pop().unwrap();
                    frame.env.borrow_mut().insert(id, val);
                    self.stack.push(Value::Symbol(id)); // define returns symbol
                }
                OpCode::Pop => {
                    self.stack.pop();
                }
                OpCode::JumpIfFalse(offset) => {
                    let val = self.stack.last().unwrap();
                    let is_false = matches!(val, Value::Boolean(false));
                    if is_false {
                        frame.ip = offset;
                    }
                }
                OpCode::Jump(offset) => {
                    frame.ip = offset;
                }
                OpCode::Call(arg_count) => {
                    let callee = self.stack[self.stack.len() - arg_count - 1].clone();
                    match callee {
                        Value::Closure {
                            params,
                            chunk,
                            env: c_env,
                        } => {
                            let mut call_env = Env::new(Some(c_env));

                            let mut has_rest = false;
                            for (i, id) in params.iter().enumerate() {
                                if lookup(*id).starts_with('&') {
                                    let rest_args =
                                        self.stack.split_off(self.stack.len() - (arg_count - i));
                                    let name = &lookup(*id)[1..];
                                    call_env.insert(intern(name), Value::List(rest_args));
                                    has_rest = true;
                                    break;
                                } else {
                                    let arg_idx = self.stack.len() - arg_count + i;
                                    // BUG: This code breaks
                                    // code: (display (map (lambda (x) (* x x) '(2 4 6))))
                                    // output: thread 'main' panicked at src/main.rs:1577:68:
                                    //         index out of bounds: the len is 3 but the index is 3
                                    //         note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
                                    call_env.insert(*id, self.stack[arg_idx].clone());
                                }
                            }

                            if !has_rest {
                                self.stack.truncate(self.stack.len() - arg_count);
                            }
                            self.stack.pop(); // pop callee

                            frames.push(CallFrame {
                                chunk,
                                ip: 0,
                                env: Rc::new(RefCell::new(call_env)),
                            });
                        }
                        Value::NativeFunction(f) => {
                            let mut args = Vec::with_capacity(arg_count);
                            let start = self.stack.len() - arg_count;
                            args.extend(self.stack.drain(start..));
                            self.stack.pop(); // pop callee

                            let result = f(args, frame.env.clone())?;
                            self.stack.push(result);
                        }
                        Value::Macro { .. } => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic("Cannot call macro at runtime".into()),
                            });
                        }
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Attempt to call non-function value: {}",
                                    callee.display()
                                )),
                            });
                        }
                    }
                }
                OpCode::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    frames.pop();
                    self.stack.push(result);
                    if frames.is_empty() {
                        return Ok(self.stack.pop().unwrap());
                    }
                }
                OpCode::BuildEnv(ids) => {
                    let mut let_env = Env::new(Some(frame.env.clone()));
                    let start = self.stack.len() - ids.len();
                    let vals: Vec<Value> = self.stack.drain(start..).collect();
                    for (id, val) in ids.into_iter().zip(vals) {
                        let_env.insert(id, val);
                    }
                    frame.env = Rc::new(RefCell::new(let_env));
                }
                OpCode::PopEnv => {
                    let parent = frame.env.borrow().parent.clone().unwrap();
                    frame.env = parent;
                }
                OpCode::MakeClosure(idx) => {
                    if let Value::Closure { params, chunk, .. } = frame.chunk.constants[idx].clone()
                    {
                        let closure = Value::Closure {
                            params,
                            chunk,
                            env: frame.env.clone(),
                        };
                        self.stack.push(closure);
                    }
                }
                OpCode::MakeMacro(id, idx) => {
                    if let Value::Macro { params, chunk, .. } = frame.chunk.constants[idx].clone() {
                        let mac = Value::Macro {
                            params,
                            chunk,
                            env: frame.env.clone(),
                        };
                        frame.env.borrow_mut().insert(id, mac.clone());
                        self.stack.push(Value::Symbol(id));
                    }
                }
                OpCode::MakeList(count) => {
                    let mut items = Vec::with_capacity(count);
                    let start = self.stack.len() - count;
                    items.extend(self.stack.drain(start..));
                    self.stack.push(Value::List(items));
                }
                OpCode::ConcatList(count) => {
                    let mut items = Vec::new();
                    let start = self.stack.len() - count;
                    for val in self.stack.drain(start..) {
                        match val {
                            Value::List(l) => items.extend(l),
                            Value::Nil => {}
                            _ => {
                                return Err(SelError {
                                    loc: Loc::default(),
                                    kind: SelErrorKind::Generic(
                                        "unquote-splicing requires a list".into(),
                                    ),
                                });
                            }
                        }
                    }
                    self.stack.push(Value::List(items));
                }
            }
        }
    }
}

pub fn macro_expand(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Ast> {
    match ast {
        Ast::List(list) => {
            if list.is_empty() {
                return Ok(Ast::List(list));
            }
            if let Ast::Symbol(loc, id) = list[0].clone() {
                let macro_opt = env.borrow().get(id);
                if let Some(Value::Macro {
                    params,
                    chunk,
                    env: m_env,
                }) = macro_opt
                {
                    let mut list_iter = list.into_iter();
                    list_iter.next(); // skip head
                    let args_ast: Vec<Ast> = list_iter.collect();
                    let mut args: Vec<Value> = Vec::new();
                    for a in args_ast {
                        args.push(ast_to_value(a));
                    }

                    let mut call_env = Env::new(Some(m_env));

                    for (i, pid) in params.iter().enumerate() {
                        if lookup(*pid).starts_with('&') {
                            let rest_args = args.split_off(i);
                            let name = &lookup(*pid)[1..];
                            call_env.insert(intern(name), Value::List(rest_args));
                            break;
                        } else {
                            if i < args.len() {
                                call_env.insert(*pid, args[i].clone());
                            } else {
                                return Err(SelError {
                                    loc: loc.clone(),
                                    kind: SelErrorKind::Generic(
                                        "Arity mismatch in macro call".into(),
                                    ),
                                });
                            }
                        }
                    }

                    let mut vm = VM::new();
                    let result_val = vm.run(chunk, Rc::new(RefCell::new(call_env)))?;

                    let expanded_ast = value_to_ast(result_val, loc.clone())?;
                    return macro_expand(expanded_ast, env);
                }
            }

            let mut expanded_list = Vec::new();
            for item in list {
                expanded_list.push(macro_expand(item, env.clone())?);
            }
            Ok(Ast::List(expanded_list))
        }
        Ast::Begin(loc, exprs) => {
            let mut exp = Vec::new();
            for e in exprs {
                exp.push(macro_expand(e, env.clone())?);
            }
            Ok(Ast::Begin(loc, exp))
        }
        Ast::If(loc, cond, t, f) => {
            let econd = macro_expand(*cond, env.clone())?;
            let et = macro_expand(*t, env.clone())?;
            let ef = if let Some(f) = f {
                Some(Box::new(macro_expand(*f, env)?))
            } else {
                None
            };
            Ok(Ast::If(loc, Box::new(econd), Box::new(et), ef))
        }
        Ast::Define(loc, id, expr) => Ok(Ast::Define(loc, id, Box::new(macro_expand(*expr, env)?))),
        Ast::Set(loc, id, expr) => Ok(Ast::Set(loc, id, Box::new(macro_expand(*expr, env)?))),
        Ast::Let(loc, bindings, body) => {
            let mut exp_b = Vec::new();
            for (id, v) in bindings {
                exp_b.push((id, macro_expand(v, env.clone())?));
            }
            let mut exp_body = Vec::new();
            for b in body {
                exp_body.push(macro_expand(b, env.clone())?);
            }
            Ok(Ast::Let(loc, exp_b, exp_body))
        }
        Ast::Lambda(loc, params, body) => {
            let mut exp_body = Vec::new();
            for b in body {
                exp_body.push(macro_expand(b, env.clone())?);
            }
            Ok(Ast::Lambda(loc, params, exp_body))
        }
        Ast::DefMacro(loc, id, expr) => {
            Ok(Ast::DefMacro(loc, id, Box::new(macro_expand(*expr, env)?)))
        }
        Ast::Quasiquote(loc, expr) => Ok(Ast::Quasiquote(
            loc,
            Box::new(macro_expand_quasiquote(*expr, env)?),
        )),
        _ => Ok(ast),
    }
}

pub fn macro_expand_quasiquote(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Ast> {
    match ast {
        Ast::Unquote(loc, expr) => Ok(Ast::Unquote(loc, Box::new(macro_expand(*expr, env)?))),
        Ast::UnquoteSplicing(loc, expr) => Ok(Ast::UnquoteSplicing(
            loc,
            Box::new(macro_expand(*expr, env)?),
        )),
        Ast::List(list) => {
            let mut exp = Vec::new();
            for item in list {
                exp.push(macro_expand_quasiquote(item, env.clone())?);
            }
            Ok(Ast::List(exp))
        }
        _ => Ok(ast),
    }
}

pub fn read_all(line: &str) -> Result<Vec<Ast>> {
    let mut lex = Lexer::new(line);
    let mut tokens = Vec::new();
    while let Some(t) = lex.next_token()? {
        tokens.push(t);
    }
    let mut pos = 0;
    let mut asts = Vec::new();
    while pos < tokens.len() {
        asts.push(parse_expr(&tokens, &mut pos)?);
    }
    Ok(asts)
}

pub fn execute_asts(asts: Vec<Ast>, env: Rc<RefCell<Env>>) -> Result<Value> {
    let mut last_val = Value::Nil;
    for ast in asts {
        let expanded = macro_expand(ast, env.clone())?;
        let mut chunk = Chunk::new();
        let mut compiler = Compiler::new(&mut chunk);
        compiler.compile(expanded)?;
        let mut vm = VM::new();
        last_val = vm.run(Rc::new(chunk), env.clone())?;
    }
    Ok(last_val)
}

fn print(val: Value) -> Result<()> {
    println!("{}", format_value(&val));
    Ok(())
}

mod sel_core {
    use crate::Result;
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::{Env, Loc, SelError, SelErrorKind, Value};

    pub fn sum(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        let mut int_sum = 0;
        let mut float_sum = 0.0;
        let mut is_float = false;

        for arg in args {
            match arg {
                Value::Integer(i) => {
                    if is_float {
                        float_sum += i as f64;
                    } else {
                        int_sum += i;
                    }
                }
                Value::Float(f) => {
                    if !is_float {
                        is_float = true;
                        float_sum = int_sum as f64 + f;
                    } else {
                        float_sum += f;
                    }
                }
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(
                            "Invalid argument to +: expected number".into(),
                        ),
                    });
                }
            }
        }
        if is_float {
            Ok(Value::Float(float_sum))
        } else {
            Ok(Value::Integer(int_sum))
        }
    }

    pub fn sub(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.is_empty() {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::Generic("Expected at least 1 argument to -".into()),
            });
        }
        let mut is_float = false;
        let mut int_val = 0;
        let mut float_val = 0.0;

        match args[0] {
            Value::Integer(i) => int_val = i,
            Value::Float(f) => {
                is_float = true;
                float_val = f;
            }
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("Invalid argument to -: expected number".into()),
                });
            }
        }

        if args.len() == 1 {
            return if is_float {
                Ok(Value::Float(-float_val))
            } else {
                Ok(Value::Integer(-int_val))
            };
        }

        for arg in args.into_iter().skip(1) {
            match arg {
                Value::Integer(i) => {
                    if is_float {
                        float_val -= i as f64;
                    } else {
                        int_val -= i;
                    }
                }
                Value::Float(f) => {
                    if !is_float {
                        is_float = true;
                        float_val = int_val as f64 - f;
                    } else {
                        float_val -= f;
                    }
                }
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(
                            "Invalid argument to -: expected number".into(),
                        ),
                    });
                }
            }
        }
        if is_float {
            Ok(Value::Float(float_val))
        } else {
            Ok(Value::Integer(int_val))
        }
    }

    pub fn mul(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        let mut int_val = 1;
        let mut float_val = 1.0;
        let mut is_float = false;

        for arg in args {
            match arg {
                Value::Integer(i) => {
                    if is_float {
                        float_val *= i as f64;
                    } else {
                        int_val *= i;
                    }
                }
                Value::Float(f) => {
                    if !is_float {
                        is_float = true;
                        float_val = int_val as f64 * f;
                    } else {
                        float_val *= f;
                    }
                }
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(
                            "Invalid argument to *: expected number".into(),
                        ),
                    });
                }
            }
        }
        if is_float {
            Ok(Value::Float(float_val))
        } else {
            Ok(Value::Integer(int_val))
        }
    }

    pub fn div(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.is_empty() {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::Generic("Expected at least 1 argument to /".into()),
            });
        }

        if args.len() == 1 {
            match args[0] {
                Value::Integer(i) => return Ok(Value::Float(1.0 / i as f64)),
                Value::Float(f) => return Ok(Value::Float(1.0 / f)),
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(
                            "Invalid argument to /: expected number".into(),
                        ),
                    });
                }
            }
        }

        let mut float_val = match args[0] {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("Invalid argument to /: expected number".into()),
                });
            }
        };

        for arg in args.into_iter().skip(1) {
            match arg {
                Value::Integer(i) => float_val /= i as f64,
                Value::Float(f) => float_val /= f,
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(
                            "Invalid argument to /: expected number".into(),
                        ),
                    });
                }
            }
        }
        Ok(Value::Float(float_val))
    }

    pub fn modulo(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 2,
                    actual: args.len(),
                },
            });
        }
        let a = match args[0] {
            Value::Integer(i) => i,
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("modulo requires integer".into()),
                });
            }
        };
        let b = match args[1] {
            Value::Integer(i) => i,
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("modulo requires integer".into()),
                });
            }
        };
        Ok(Value::Integer(a % b))
    }

    fn compare_nums(args: Vec<Value>, op: fn(f64, f64) -> bool) -> Result<Value> {
        if args.len() < 2 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::Generic(
                    "comparison requires at least 2 arguments".into(),
                ),
            });
        }
        let mut prev = match args[0] {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("comparison requires numbers".into()),
                });
            }
        };
        for arg in args.into_iter().skip(1) {
            let curr = match arg {
                Value::Integer(i) => i as f64,
                Value::Float(f) => f,
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic("comparison requires numbers".into()),
                    });
                }
            };
            if !op(prev, curr) {
                return Ok(Value::Boolean(false));
            }
            prev = curr;
        }
        Ok(Value::Boolean(true))
    }

    pub fn num_noteq(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        compare_nums(args, |a, b| a != b)
    }
    pub fn num_eq(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        compare_nums(args, |a, b| a == b)
    }
    pub fn num_lt(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        compare_nums(args, |a, b| a < b)
    }
    pub fn num_gt(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        compare_nums(args, |a, b| a > b)
    }
    pub fn num_lte(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        compare_nums(args, |a, b| a <= b)
    }
    pub fn num_gte(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        compare_nums(args, |a, b| a >= b)
    }

    pub fn cons(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 2,
                    actual: args.len(),
                },
            });
        }
        let tail = args.pop().unwrap();
        let head = args.pop().unwrap();
        match tail {
            Value::List(l) => {
                let mut new_l = vec![head];
                new_l.extend(l);
                Ok(Value::List(new_l))
            }
            Value::Nil => Ok(Value::List(vec![head])),
            _ => Ok(Value::List(vec![head, tail])),
        }
    }

    pub fn car(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        match args.pop().unwrap() {
            Value::List(mut l) => {
                if l.is_empty() {
                    return Err(crate::SelError {
                        loc: crate::Loc::default(),
                        kind: crate::SelErrorKind::Generic("car on empty list".into()),
                    });
                }
                Ok(l.remove(0))
            }
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("car requires a list".into()),
                });
            }
        }
    }

    pub fn nth(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 2,
                    actual: args.len(),
                },
            });
        }
        let index = args.pop().unwrap();
        match args.pop().unwrap() {
            Value::List(mut l) => match index {
                Value::Integer(index) => {
                    if (index as usize) < l.len() {
                        Ok(l.remove(index as usize))
                    } else {
                        Ok(Value::Nil)
                    }
                }
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic("nth requires a interger".into()),
                    });
                }
            },
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("nth requires a list".into()),
                });
            }
        }
    }

    pub fn cdr(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        match args.pop().unwrap() {
            Value::List(mut l) => {
                if l.is_empty() {
                    return Err(crate::SelError {
                        loc: crate::Loc::default(),
                        kind: crate::SelErrorKind::Generic("cdr on empty list".into()),
                    });
                }
                l.remove(0);
                Ok(if l.is_empty() {
                    Value::Nil
                } else {
                    Value::List(l)
                })
            }
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("cdr requires a list".into()),
                });
            }
        }
    }

    pub fn count(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        match args.pop().unwrap() {
            Value::List(l) => Ok(Value::Integer(l.len() as _)),
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("count requires a list".into()),
                });
            }
        }
    }

    pub fn empty(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        match args.pop().unwrap() {
            Value::List(l) => Ok(Value::Boolean(l.is_empty())),
            Value::Nil => Ok(Value::Boolean(true)),
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("empty requires a list".into()),
                });
            }
        }
    }

    pub fn list(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.is_empty() {
            Ok(Value::Nil)
        } else {
            Ok(Value::List(args))
        }
    }

    pub fn is_nil(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        match &args[0] {
            Value::Nil => Ok(Value::Boolean(true)),
            Value::List(l) if l.is_empty() => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn is_list(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        match &args[0] {
            Value::List(l) if !l.is_empty() => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn is_number(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        Ok(Value::Boolean(matches!(
            args[0],
            Value::Integer(_) | Value::Float(_)
        )))
    }

    pub fn is_string(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        Ok(Value::Boolean(matches!(args[0], Value::String(_))))
    }

    pub fn is_function(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        Ok(Value::Boolean(matches!(
            args[0],
            Value::NativeFunction(_) | Value::Closure { .. }
        )))
    }

    pub fn not(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError {
                loc: crate::Loc::default(),
                kind: crate::SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        match args[0] {
            Value::Boolean(false) => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn print_func(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        for arg in args {
            print!("{} ", crate::format_value(&arg));
        }
        println!();
        Ok(Value::Nil)
    }

    pub fn display(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        for arg in args {
            match arg {
                Value::String(s) => print!("{}", s),
                _ => print!("{}", arg),
            }
        }
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        Ok(Value::Nil)
    }

    pub fn newline(_args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        println!();
        Ok(Value::Nil)
    }

    pub fn ffi_dlopen(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(SelError {
                loc: Loc::default(),
                kind: SelErrorKind::ArityMismatch {
                    expected: 1,
                    actual: args.len(),
                },
            });
        }
        if let Value::String(s) = &args[0] {
            unsafe {
                match libloading::Library::new(s) {
                    Ok(lib) => Ok(Value::Library(Rc::new(lib))),
                    Err(e) => Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(format!("dlopen failed: {}", e)),
                    }),
                }
            }
        } else {
            Err(SelError {
                loc: Loc::default(),
                kind: SelErrorKind::Generic("ffi-dlopen requires a string".into()),
            })
        }
    }

    pub fn ffi_dlsym(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            return Err(SelError {
                loc: Loc::default(),
                kind: SelErrorKind::ArityMismatch {
                    expected: 2,
                    actual: args.len(),
                },
            });
        }
        let lib = match &args[0] {
            Value::Library(l) => l,
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("ffi-dlsym requires a library".into()),
                });
            }
        };
        let sym_name = match &args[1] {
            Value::String(s) => s,
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("ffi-dlsym requires a string symbol name".into()),
                });
            }
        };

        let mut sym_bytes = sym_name.as_bytes().to_vec();
        sym_bytes.push(0);

        unsafe {
            match lib.get::<*const ()>(&sym_bytes) {
                Ok(sym) => {
                    let ptr = *sym as usize;
                    Ok(Value::Pointer(ptr))
                }
                Err(e) => Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic(format!("dlsym failed: {}", e)),
                }),
            }
        }
    }

    pub fn ffi_call(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() < 3 {
            return Err(SelError {
                loc: Loc::default(),
                kind: SelErrorKind::Generic(
                    "ffi-call requires at least ptr, ret_type, arg_types".into(),
                ),
            });
        }

        let ptr = match args[0] {
            Value::Pointer(p) => p,
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("ffi-call requires a pointer".into()),
                });
            }
        };

        let ret_type_sym = match args[1] {
            Value::Symbol(s) => crate::lookup(s),
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("ffi-call requires a return type symbol".into()),
                });
            }
        };

        let arg_type_syms = match &args[2] {
            Value::List(l) => {
                let mut syms = Vec::new();
                for v in l {
                    if let Value::Symbol(s) = v {
                        syms.push(crate::lookup(*s));
                    } else {
                        return Err(SelError {
                            loc: Loc::default(),
                            kind: SelErrorKind::Generic(
                                "arg_types must be a list of symbols".into(),
                            ),
                        });
                    }
                }
                syms
            }
            Value::Nil => Vec::new(),
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic("arg_types must be a list".into()),
                });
            }
        };

        if args.len() - 3 != arg_type_syms.len() {
            return Err(SelError {
                loc: Loc::default(),
                kind: SelErrorKind::Generic(format!(
                    "arg_types length mismatch: expected {}, got {}",
                    arg_type_syms.len(),
                    args.len() - 3
                )),
            });
        }

        let ret_type = match ret_type_sym.as_str() {
            "void" => libffi::middle::Type::void(),
            "i32" => libffi::middle::Type::i32(),
            "i64" => libffi::middle::Type::i64(),
            "u32" => libffi::middle::Type::u32(),
            "u64" => libffi::middle::Type::u64(),
            "f32" => libffi::middle::Type::f32(),
            "f64" => libffi::middle::Type::f64(),
            "bool" => libffi::middle::Type::u8(),
            "*u8" => libffi::middle::Type::pointer(),
            _ => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic(format!(
                        "Unsupported return type: {}",
                        ret_type_sym
                    )),
                });
            }
        };

        let mut arg_types = Vec::new();
        for sym in &arg_type_syms {
            let t = match sym.as_str() {
                "i32" => libffi::middle::Type::i32(),
                "i64" => libffi::middle::Type::i64(),
                "u32" => libffi::middle::Type::u32(),
                "u64" => libffi::middle::Type::u64(),
                "f32" => libffi::middle::Type::f32(),
                "f64" => libffi::middle::Type::f64(),
                "bool" => libffi::middle::Type::u8(),
                "*u8" => libffi::middle::Type::pointer(),
                _ => {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(format!("Unsupported arg type: {}", sym)),
                    });
                }
            };
            arg_types.push(t);
        }

        let cif = libffi::middle::Cif::new(arg_types.into_iter(), ret_type);

        let mut c_strings = Vec::new();

        enum FfiArg {
            I32(i32),
            I64(i64),
            U8(u8),
            U32(u32),
            U64(u64),
            F32(f32),
            F64(f64),
            Ptr(*const std::ffi::c_void),
        }

        let mut ffi_args_storage = Vec::new();

        for (i, arg_val) in args.iter().skip(3).enumerate() {
            let sym = &arg_type_syms[i];
            match sym.as_str() {
                "i32" => {
                    let v = match arg_val {
                        Value::Integer(n) => *n as i32,
                        Value::Float(f) => *f as i32,
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected integer for arg {}",
                                    i
                                )),
                            });
                        }
                    };
                    ffi_args_storage.push(FfiArg::I32(v));
                }
                "bool" => {
                    let v = match arg_val {
                        Value::Boolean(b) => if *b { 1u8 } else { 0u8 },
                        Value::Integer(n) => if *n != 0 { 1u8 } else { 0u8 },
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected boolean for arg {}",
                                    i
                                )),
                            });
                        }
                    };
                    ffi_args_storage.push(FfiArg::U8(v));
                }
                "i64" => {
                    let v = match arg_val {
                        Value::Integer(n) => *n,
                        Value::Float(f) => *f as i64,
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected integer for arg {}",
                                    i
                                )),
                            });
                        }
                    };
                    ffi_args_storage.push(FfiArg::I64(v));
                }
                "u32" => {
                    let v = match arg_val {
                        Value::Integer(n) => *n as u32,
                        Value::Float(f) => *f as u32,
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected integer for arg {}",
                                    i
                                )),
                            });
                        }
                    };
                    ffi_args_storage.push(FfiArg::U32(v));
                }
                "u64" => {
                    let v = match arg_val {
                        Value::Integer(n) => *n as u64,
                        Value::Float(f) => *f as u64,
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected integer for arg {}",
                                    i
                                )),
                            });
                        }
                    };
                    ffi_args_storage.push(FfiArg::U64(v));
                }
                "f32" => {
                    let v = match arg_val {
                        Value::Integer(n) => *n as f32,
                        Value::Float(f) => *f as f32,
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected float for arg {}",
                                    i
                                )),
                            });
                        }
                    };
                    ffi_args_storage.push(FfiArg::F32(v));
                }
                "f64" => {
                    let v = match arg_val {
                        Value::Integer(n) => *n as f64,
                        Value::Float(f) => *f,
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected float for arg {}",
                                    i
                                )),
                            });
                        }
                    };
                    ffi_args_storage.push(FfiArg::F64(v));
                }
                "*u8" => {
                    match arg_val {
                        Value::String(s) => {
                            let cstr = std::ffi::CString::new(s.as_str()).unwrap();
                            let ptr = cstr.as_ptr() as *const std::ffi::c_void;
                            c_strings.push(cstr); // keep alive
                            ffi_args_storage.push(FfiArg::Ptr(ptr));
                        }
                        Value::Pointer(p) => {
                            ffi_args_storage.push(FfiArg::Ptr(*p as *const std::ffi::c_void));
                        }
                        Value::Nil => {
                            ffi_args_storage.push(FfiArg::Ptr(std::ptr::null()));
                        }
                        _ => {
                            return Err(SelError {
                                loc: Loc::default(),
                                kind: SelErrorKind::Generic(format!(
                                    "Expected string or pointer for arg {}",
                                    i
                                )),
                            });
                        }
                    }
                }
                _ => unreachable!(),
            }
        }

        let mut call_args = Vec::new();
        for arg in &ffi_args_storage {
            match arg {
                FfiArg::I32(v) => call_args.push(libffi::middle::arg(v)),
                FfiArg::I64(v) => call_args.push(libffi::middle::arg(v)),
                FfiArg::U8(v) => call_args.push(libffi::middle::arg(v)),
                FfiArg::U32(v) => call_args.push(libffi::middle::arg(v)),
                FfiArg::U64(v) => call_args.push(libffi::middle::arg(v)),
                FfiArg::F32(v) => call_args.push(libffi::middle::arg(v)),
                FfiArg::F64(v) => call_args.push(libffi::middle::arg(v)),
                FfiArg::Ptr(v) => call_args.push(libffi::middle::arg(v)),
            }
        }

        let code_ptr = libffi::middle::CodePtr::from_ptr(ptr as *mut _);

        unsafe {
            match ret_type_sym.as_str() {
                "void" => {
                    cif.call::<()>(code_ptr, &call_args);
                    Ok(Value::Nil)
                }
                "bool" => {
                    let res: u8 = cif.call(code_ptr, &call_args);
                    Ok(Value::Boolean(res != 0))
                }
                "i32" => {
                    let res: i32 = cif.call(code_ptr, &call_args);
                    Ok(Value::Integer(res as i64))
                }
                "i64" => {
                    let res: i64 = cif.call(code_ptr, &call_args);
                    Ok(Value::Integer(res))
                }
                "u32" => {
                    let res: u32 = cif.call(code_ptr, &call_args);
                    Ok(Value::Integer(res as i64))
                }
                "u64" => {
                    let res: u64 = cif.call(code_ptr, &call_args);
                    Ok(Value::Integer(res as i64))
                }
                "f32" => {
                    let res: f32 = cif.call(code_ptr, &call_args);
                    Ok(Value::Float(res as f64))
                }
                "f64" => {
                    let res: f64 = cif.call(code_ptr, &call_args);
                    Ok(Value::Float(res))
                }
                "*u8" => {
                    let res: *const std::ffi::c_char = cif.call(code_ptr, &call_args);
                    if res.is_null() {
                        Ok(Value::Nil)
                    } else {
                        let c_str = std::ffi::CStr::from_ptr(res);
                        Ok(Value::String(c_str.to_string_lossy().into_owned()))
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn load(env: Rc<RefCell<Env>>) {
        let mut e = env.borrow_mut();
        e.insert(crate::intern("+"), Value::NativeFunction(sum));
        e.insert(crate::intern("-"), Value::NativeFunction(sub));
        e.insert(crate::intern("*"), Value::NativeFunction(mul));
        e.insert(crate::intern("/"), Value::NativeFunction(div));
        e.insert(crate::intern("mod"), Value::NativeFunction(modulo));

        e.insert(crate::intern("="), Value::NativeFunction(num_eq));
        e.insert(crate::intern("!="), Value::NativeFunction(num_noteq));
        e.insert(crate::intern("<"), Value::NativeFunction(num_lt));
        e.insert(crate::intern(">"), Value::NativeFunction(num_gt));
        e.insert(crate::intern("<="), Value::NativeFunction(num_lte));
        e.insert(crate::intern(">="), Value::NativeFunction(num_gte));

        e.insert(crate::intern("cons"), Value::NativeFunction(cons));
        e.insert(crate::intern("car"), Value::NativeFunction(car));
        e.insert(crate::intern("cdr"), Value::NativeFunction(cdr));
        e.insert(crate::intern("nth"), Value::NativeFunction(nth));
        e.insert(crate::intern("count"), Value::NativeFunction(count));
        e.insert(crate::intern("list"), Value::NativeFunction(list));
        e.insert(crate::intern("empty?"), Value::NativeFunction(empty));

        e.insert(crate::intern("nil?"), Value::NativeFunction(is_nil));
        e.insert(crate::intern("list?"), Value::NativeFunction(is_list));
        e.insert(crate::intern("number?"), Value::NativeFunction(is_number));
        e.insert(crate::intern("string?"), Value::NativeFunction(is_string));
        e.insert(
            crate::intern("function?"),
            Value::NativeFunction(is_function),
        );

        e.insert(crate::intern("not"), Value::NativeFunction(not));
        e.insert(crate::intern("print"), Value::NativeFunction(print_func));
        e.insert(crate::intern("display"), Value::NativeFunction(display));
        e.insert(crate::intern("newline"), Value::NativeFunction(newline));

        e.insert(
            crate::intern("ffi-dlopen"),
            Value::NativeFunction(ffi_dlopen),
        );
        e.insert(crate::intern("ffi-dlsym"), Value::NativeFunction(ffi_dlsym));
        e.insert(crate::intern("ffi-call"), Value::NativeFunction(ffi_call));
    }
}
