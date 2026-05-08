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
            SelErrorKind::InvalidNumber(s) => write!(f, "{}: Invalid number format `{}`", self.loc, s),
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
        let program = format!("(begin {core_src})");
        match read(&program) {
            Ok(ast) => {
                if let Err(e) = eval(ast, env.clone()) {
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

        let program = format!("(begin {src})");
        let ast = read(&program)?;
        eval(ast, env).map_err(|e| anyhow::anyhow!("{}", e)).map(|_| ())
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

                let ast = match read(line) {
                    Ok(ast) => ast,
                    Err(e) => {
                        println!("Error: {e}");
                        continue;
                    }
                };
                let val = match eval(ast, env.clone()) {
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
    Number,
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

                if ident.parse::<i64>().is_ok() || ident.parse::<f64>().is_ok() {
                    Ok(Some(Token {
                        kind: TokenKind::Number,
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
        TokenKind::Number => {
            if let Ok(i) = t.source.parse::<i64>() {
                Ok(Ast::Integer(i))
            } else if let Ok(f) = t.source.parse::<f64>() {
                Ok(Ast::Float(f))
            } else {
                Err(SelError {
                    loc: t.loc,
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
                        })
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
                                })
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
                            kind: SelErrorKind::Generic("Expected parameter list in defmacro".into()),
                        })
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
                                    kind: SelErrorKind::Generic("Expected binding pair in let".into()),
                                });
                            }
                        }
                    }
                    Ast::Nil => {}
                    _ => {
                        return Err(SelError {
                            loc: s_loc.clone(),
                            kind: SelErrorKind::Generic("Expected binding list in let".into()),
                        })
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
enum Value {
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
        body: Ast,
        env: Rc<RefCell<Env>>,
    },
    Macro {
        params: Vec<u32>,
        body: Ast,
        env: Rc<RefCell<Env>>,
    },
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
#[allow(unused)]
enum Ast {
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

fn eval_quasiquote(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Value> {
    match ast {
        Ast::Unquote(_, expr) => eval(*expr, env),
        Ast::UnquoteSplicing(loc, _) => Err(SelError {
            loc,
            kind: SelErrorKind::Generic("unquote-splicing invalid at top level of quasiquote".into()),
        }),
        Ast::List(list) => {
            let mut result = Vec::new();
            for item in list {
                if let Ast::UnquoteSplicing(loc, expr) = item {
                    let val = eval(*expr, env.clone())?;
                    match val {
                        Value::List(items) => result.extend(items),
                        Value::Nil => {}
                        _ => {
                            return Err(SelError {
                                loc,
                                kind: SelErrorKind::Generic("unquote-splicing requires a list".into()),
                            });
                        }
                    }
                } else {
                    result.push(eval_quasiquote(item, env.clone())?);
                }
            }
            Ok(Value::List(result))
        }
        _ => Ok(ast_to_value(ast)),
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
        Ast::Define(_, id, val) => {
            Value::List(vec![Value::Symbol(intern("define")), Value::Symbol(id), ast_to_value(*val)])
        }
        Ast::DefMacro(_, id, val) => {
            Value::List(vec![Value::Symbol(intern("defmacro")), Value::Symbol(id), ast_to_value(*val)])
        }
        Ast::Set(_, id, val) => {
            Value::List(vec![Value::Symbol(intern("set!")), Value::Symbol(id), ast_to_value(*val)])
        }
        Ast::If(_, cond, t, f) => {
            let mut list = vec![Value::Symbol(intern("if")), ast_to_value(*cond), ast_to_value(*t)];
            if let Some(f) = f {
                list.push(ast_to_value(*f));
            }
            Value::List(list)
        }
        Ast::Lambda(_, params, body) => {
            let mut list = vec![Value::Symbol(intern("lambda")), Value::List(params.into_iter().map(Value::Symbol).collect())];
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
        Ast::Quasiquote(_, val) => Value::List(vec![Value::Symbol(intern("quasiquote")), ast_to_value(*val)]),
        Ast::Unquote(_, val) => Value::List(vec![Value::Symbol(intern("unquote")), ast_to_value(*val)]),
        Ast::UnquoteSplicing(_, val) => Value::List(vec![Value::Symbol(intern("unquote-splicing")), ast_to_value(*val)]),
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

fn bind_args(
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

fn eval(mut ast: Ast, mut env: Rc<RefCell<Env>>) -> Result<Value> {
    loop {
        match std::mem::replace(&mut ast, Ast::Nil) {
            Ast::Symbol(loc, id) => {
                if let Some(v) = env.borrow().get(id) {
                    return Ok(v.clone());
                } else {
                    return Err(SelError {
                        loc,
                        kind: SelErrorKind::UndefinedVariable(id),
                    });
                }
            }
            Ast::Nil => return Ok(Value::Nil),
            Ast::Integer(i) => return Ok(Value::Integer(i)),
            Ast::Float(f) => return Ok(Value::Float(f)),
            Ast::String(s) => return Ok(Value::String(s)),
            Ast::Boolean(b) => return Ok(Value::Boolean(b)),
            Ast::Bind(id) => {
                return Err(SelError {
                    loc: Loc::default(),
                    kind: SelErrorKind::Generic(format!("Unexpected `&{}`.", lookup(id))),
                })
            }

            Ast::Define(_loc, id, expr) => {
                let val = eval(*expr, env.clone())?;
                env.borrow_mut().insert(id, val.clone());
                return Ok(val);
            }
            Ast::DefMacro(loc, id, expr) => {
                let val = eval(*expr, env.clone())?;
                if let Value::Closure { params, body, env: c_env } = val {
                    let macro_val = Value::Macro { params, body, env: c_env };
                    env.borrow_mut().insert(id, macro_val.clone());
                    return Ok(macro_val);
                } else {
                    return Err(SelError {
                        loc,
                        kind: SelErrorKind::Generic("defmacro expects a lambda".into()),
                    });
                }
            }
            Ast::Set(loc, id, expr) => {
                let val = eval(*expr, env.clone())?;
                if !env.borrow_mut().set(id, val.clone()) {
                    return Err(SelError {
                        loc,
                        kind: SelErrorKind::UnboundVariable(id),
                    });
                }
                return Ok(val);
            }
            Ast::If(_loc, cond_ast, true_ast, false_ast) => {
                let cond = eval(*cond_ast, env.clone())?;
                let is_true = match cond {
                    Value::Boolean(false) => false,
                    _ => true,
                };
                if is_true {
                    ast = *true_ast;
                } else if let Some(f) = false_ast {
                    ast = *f;
                } else {
                    return Ok(Value::Nil);
                }
            }
            Ast::Begin(_loc, mut exprs) => {
                if exprs.is_empty() {
                    return Ok(Value::Nil);
                }
                let last = exprs.pop().unwrap();
                for expr in exprs {
                    eval(expr, env.clone())?;
                }
                ast = last;
            }
            Ast::Let(_loc, bindings, mut body) => {
                let mut let_env = Env::new(Some(env.clone()));
                for (id, val_ast) in bindings {
                    let val = eval(val_ast, env.clone())?;
                    let_env.insert(id, val);
                }
                env = Rc::new(RefCell::new(let_env));
                if body.is_empty() {
                    return Ok(Value::Nil);
                }
                let last = body.pop().unwrap();
                for expr in body {
                    eval(expr, env.clone())?;
                }
                ast = last;
            }
            Ast::Lambda(loc, params, body_asts) => {
                let body = if body_asts.len() == 1 {
                    body_asts.into_iter().next().unwrap()
                } else {
                    Ast::Begin(loc, body_asts)
                };
                return Ok(Value::Closure {
                    params,
                    body,
                    env: env.clone(),
                });
            }
            Ast::Quote(_, expr) => return Ok(ast_to_value(*expr)),
            Ast::Quasiquote(_, expr) => return eval_quasiquote(*expr, env),
            Ast::And(_, exprs) => {
                if exprs.is_empty() {
                    return Ok(Value::Boolean(true));
                }
                let mut iter = exprs.into_iter().peekable();
                while let Some(e) = iter.next() {
                    if iter.peek().is_none() {
                        ast = e;
                        break;
                    } else {
                        let val = eval(e, env.clone())?;
                        if let Value::Boolean(false) = val {
                            return Ok(Value::Boolean(false));
                        }
                    }
                }
            }
            Ast::Or(_, exprs) => {
                if exprs.is_empty() {
                    return Ok(Value::Boolean(false));
                }
                let mut iter = exprs.into_iter().peekable();
                while let Some(e) = iter.next() {
                    if iter.peek().is_none() {
                        ast = e;
                        break;
                    } else {
                        let val = eval(e, env.clone())?;
                        if !matches!(val, Value::Boolean(false)) {
                            return Ok(val);
                        }
                    }
                }
            }
            Ast::Unquote(loc, _) => {
                return Err(SelError {
                    loc,
                    kind: SelErrorKind::Generic("unquote invalid outside of quasiquote".into()),
                })
            }
            Ast::UnquoteSplicing(loc, _) => {
                return Err(SelError {
                    loc,
                    kind: SelErrorKind::Generic("unquote-splicing invalid outside of quasiquote".into()),
                })
            }
            Ast::List(mut list) => {
                if list.is_empty() {
                    return Ok(Value::Nil);
                }
                let head = list.remove(0);
                let head_loc = match &head {
                    Ast::Symbol(loc, _) => loc.clone(),
                    _ => Loc::default(),
                };
                let func = eval(head, env.clone())?;
                if let Value::Macro {
                    params,
                    body,
                    env: m_env,
                } = func
                {
                    let args: Vec<Value> = list.into_iter().map(ast_to_value).collect();
                    let macro_env = bind_args(&params, args, m_env, head_loc.clone())?;
                    let expanded = eval(body, macro_env)?;
                    ast = value_to_ast(expanded, head_loc)?;
                    continue;
                }

                let mut args = Vec::new();
                for arg_ast in list {
                    args.push(eval(arg_ast, env.clone())?);
                }

                match func {
                    Value::NativeFunction(f) => return f(args, env),
                    Value::Closure {
                        params,
                        body,
                        env: c_env,
                    } => {
                        env = bind_args(&params, args, c_env, head_loc)?;
                        ast = body;
                    }
                    _ => {
                        return Err(SelError {
                            loc: head_loc,
                            kind: SelErrorKind::Generic(format!(
                                "Attempt to call non-function value: {}",
                                func.display()
                            )),
                        })
                    }
                }
            }
        }
    }
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
                _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("Invalid argument to +: expected number".into()) }),
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
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::Generic("Expected at least 1 argument to -".into()) });
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
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("Invalid argument to -: expected number".into()) }),
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
                _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("Invalid argument to -: expected number".into()) }),
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
                _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("Invalid argument to *: expected number".into()) }),
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
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::Generic("Expected at least 1 argument to /".into()) });
        }

        if args.len() == 1 {
            match args[0] {
                Value::Integer(i) => return Ok(Value::Float(1.0 / i as f64)),
                Value::Float(f) => return Ok(Value::Float(1.0 / f)),
                _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("Invalid argument to /: expected number".into()) }),
            }
        }

        let mut float_val = match args[0] {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("Invalid argument to /: expected number".into()) }),
        };

        for arg in args.into_iter().skip(1) {
            match arg {
                Value::Integer(i) => float_val /= i as f64,
                Value::Float(f) => float_val /= f,
                _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("Invalid argument to /: expected number".into()) }),
            }
        }
        Ok(Value::Float(float_val))
    }

    pub fn modulo(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 2, actual: args.len() } });
        }
        let a = match args[0] {
            Value::Integer(i) => i,
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("modulo requires integer".into()) }),
        };
        let b = match args[1] {
            Value::Integer(i) => i,
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("modulo requires integer".into()) }),
        };
        Ok(Value::Integer(a % b))
    }

    fn compare_nums(args: Vec<Value>, op: fn(f64, f64) -> bool) -> Result<Value> {
        if args.len() < 2 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::Generic("comparison requires at least 2 arguments".into()) });
        }
        let mut prev = match args[0] {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("comparison requires numbers".into()) }),
        };
        for arg in args.into_iter().skip(1) {
            let curr = match arg {
                Value::Integer(i) => i as f64,
                Value::Float(f) => f,
                _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("comparison requires numbers".into()) }),
            };
            if !op(prev, curr) {
                return Ok(Value::Boolean(false));
            }
            prev = curr;
        }
        Ok(Value::Boolean(true))
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
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 2, actual: args.len() } });
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
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        match args.pop().unwrap() {
            Value::List(mut l) => {
                if l.is_empty() {
                    return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::Generic("car on empty list".into()) });
                }
                Ok(l.remove(0))
            }
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("car requires a list".into()) }),
        }
    }

    pub fn nth(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 2, actual: args.len() } });
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
                _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("nth requires a interger".into()) }),
            },
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("nth requires a list".into()) }),
        }
    }

    pub fn cdr(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        match args.pop().unwrap() {
            Value::List(mut l) => {
                if l.is_empty() {
                    return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::Generic("cdr on empty list".into()) });
                }
                l.remove(0);
                Ok(if l.is_empty() {
                    Value::Nil
                } else {
                    Value::List(l)
                })
            }
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("cdr requires a list".into()) }),
        }
    }

    pub fn count(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        match args.pop().unwrap() {
            Value::List(l) => Ok(Value::Integer(l.len() as _)),
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("count requires a list".into()) }),
        }
    }

    pub fn empty(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        match args.pop().unwrap() {
            Value::List(l) => Ok(Value::Boolean(l.is_empty())),
            Value::Nil => Ok(Value::Boolean(true)),
            _ => return Err(SelError { loc: Loc::default(), kind: SelErrorKind::Generic("empty requires a list".into()) }),
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
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        match &args[0] {
            Value::Nil => Ok(Value::Boolean(true)),
            Value::List(l) if l.is_empty() => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn is_list(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        match &args[0] {
            Value::List(l) if !l.is_empty() => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn is_number(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        Ok(Value::Boolean(matches!(
            args[0],
            Value::Integer(_) | Value::Float(_)
        )))
    }

    pub fn is_string(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        Ok(Value::Boolean(matches!(args[0], Value::String(_))))
    }

    pub fn is_function(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
        }
        Ok(Value::Boolean(matches!(
            args[0],
            Value::NativeFunction(_) | Value::Closure { .. }
        )))
    }

    pub fn not(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            return Err(crate::SelError { loc: crate::Loc::default(), kind: crate::SelErrorKind::ArityMismatch { expected: 1, actual: args.len() } });
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

    pub fn load(env: Rc<RefCell<Env>>) {
        let mut e = env.borrow_mut();
        e.insert(crate::intern("+"), Value::NativeFunction(sum));
        e.insert(crate::intern("-"), Value::NativeFunction(sub));
        e.insert(crate::intern("*"), Value::NativeFunction(mul));
        e.insert(crate::intern("/"), Value::NativeFunction(div));
        e.insert(crate::intern("mod"), Value::NativeFunction(modulo));

        e.insert(crate::intern("="), Value::NativeFunction(num_eq));
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
    }
}
