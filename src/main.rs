use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::io::stdin;
use std::io::stdout;

use std::cell::RefCell;
use std::iter::Peekable;
use std::rc::Rc;
use std::str::Chars;

#[derive(Debug, Default)]
pub struct Env {
    bindings: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Self {
            bindings: HashMap::new(),
            parent,
        }
    }

    fn get(&self, name: &str) -> Option<Value> {
        if let Some(val) = self.bindings.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().get(name)
        } else {
            None
        }
    }

    fn insert(&mut self, name: String, val: Value) {
        self.bindings.insert(name, val);
    }

    fn set(&mut self, name: &str, val: Value) -> bool {
        if self.bindings.contains_key(name) {
            self.bindings.insert(name.to_string(), val);
            true
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().set(name, val)
        } else {
            false
        }
    }
}

fn main() -> Result<()> {
    let env = Rc::new(RefCell::new(Env::default()));
    sel_core::load(env.clone());

    let mut args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let script_path = args.remove(1);

        let scheme_args = args.into_iter().skip(1).map(Value::String).collect();
        env.borrow_mut()
            .insert("*args*".to_string(), Value::List(scheme_args));

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
        eval(ast, env).map(|_| ())
    } else {
        env.borrow_mut()
            .insert("*args*".to_string(), Value::List(vec![]));
        println!("Welcome to the Sel Scheme repl. (Use `quit` to exit)");
        repl("sel> ", env)
    }
}

fn repl(prompt: &str, env: Rc<RefCell<Env>>) -> Result<()> {
    let mut line_buffer = String::new();
    loop {
        line_buffer.clear();
        print!("{prompt}");
        stdout().flush()?;
        stdin().read_line(&mut line_buffer)?;
        let line = line_buffer.trim();

        match line {
            "" => continue,
            "quit" => break,
            _ => (),
        }

        let ast = match read(line) {
            Ok(ast) => ast,
            Err(e) => {
                println!("Error: {e}");
                stdout().flush()?;
                continue;
            }
        };
        let val = match eval(ast, env.clone()) {
            Ok(val) => val,
            Err(e) => {
                println!("Error: {e}");
                stdout().flush()?;
                continue;
            }
        };
        if let Err(e) = print(val) {
            println!("Error: {e}");
            stdout().flush()?;
            continue;
        }
    }
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
                            anyhow::bail!("{}: Unterminated string escape", start_loc);
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
                anyhow::bail!("{}: Invalid character following #", start_loc);
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
                    anyhow::bail!(
                        "{}: Unexpected character '{}'",
                        start_loc,
                        self.advance().unwrap()
                    );
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
        anyhow::bail!("Unexpected EOF");
    }
    let t = &tokens[*pos];
    *pos += 1;

    match t.kind {
        TokenKind::OpenParen => {
            let mut list = Vec::new();
            while *pos < tokens.len() && tokens[*pos].kind != TokenKind::CloseParen {
                list.push(parse_expr(tokens, pos)?);
            }
            if *pos >= tokens.len() {
                anyhow::bail!("{}: Missing closing parenthesis", t.loc);
            }
            *pos += 1; // consume ')'
            Ok(Ast::List(list))
        }
        TokenKind::CloseParen => {
            anyhow::bail!("{}: Unexpected closing parenthesis", t.loc);
        }
        TokenKind::Quote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::List(vec![
                Ast::Atom(Token {
                    kind: TokenKind::Identifier,
                    source: "quote".into(),
                    loc: t.loc.clone(),
                }),
                expr,
            ]))
        }
        TokenKind::QuasiQuote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::List(vec![
                Ast::Atom(Token {
                    kind: TokenKind::Identifier,
                    source: "quasiquote".into(),
                    loc: t.loc.clone(),
                }),
                expr,
            ]))
        }
        TokenKind::Unquote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::List(vec![
                Ast::Atom(Token {
                    kind: TokenKind::Identifier,
                    source: "unquote".into(),
                    loc: t.loc.clone(),
                }),
                expr,
            ]))
        }
        TokenKind::UnquoteSplicing => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::List(vec![
                Ast::Atom(Token {
                    kind: TokenKind::Identifier,
                    source: "unquote-splicing".into(),
                    loc: t.loc.clone(),
                }),
                expr,
            ]))
        }
        TokenKind::String => Ok(Ast::String(t.source.clone())),
        TokenKind::Boolean => Ok(Ast::Boolean(t.source == "#t")),
        TokenKind::Number => {
            if let Ok(i) = t.source.parse::<i64>() {
                Ok(Ast::Integer(i))
            } else if let Ok(f) = t.source.parse::<f64>() {
                Ok(Ast::Float(f))
            } else {
                anyhow::bail!("{}: Invalid number format", t.loc)
            }
        }
        TokenKind::Identifier => match t.source.as_str() {
            "nil" => Ok(Ast::Nil),
            "define" => Ok(Ast::Define(t.loc.clone())),
            "let" => Ok(Ast::Let(t.loc.clone())),
            "set!" => Ok(Ast::Set(t.loc.clone())),
            _ => Ok(Ast::Atom(t.clone())),
        },
    }
}

#[derive(Debug, Clone)]
enum Value {
    Nil,
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Symbol(String),
    List(Vec<Value>),
    NativeFunction(fn(Vec<Value>, Rc<RefCell<Env>>) -> Result<Value>),
    Closure {
        params: Vec<String>,
        body: Ast,
        env: Rc<RefCell<Env>>,
    },
}

fn ast_to_value(ast: Ast) -> Value {
    match ast {
        Ast::Nil => Value::Nil,
        Ast::Integer(i) => Value::Integer(i),
        Ast::Float(f) => Value::Float(f),
        Ast::String(s) => Value::String(s),
        Ast::Boolean(b) => Value::Boolean(b),
        Ast::Atom(t) => Value::Symbol(t.source),
        Ast::List(l) => Value::List(l.into_iter().map(ast_to_value).collect()),
        Ast::Define(_) => Value::Symbol("define".to_string()),
        Ast::Let(_) => Value::Symbol("let".to_string()),
        Ast::Set(_) => Value::Symbol("set!".to_string()),
    }
}

#[derive(Debug, Clone)]
enum Ast {
    Define(Loc),
    Let(Loc),
    Set(Loc),
    Nil,
    Atom(Token),
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    List(Vec<Self>),
}

impl std::fmt::Display for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ast::Define(_) => write!(f, "define"),
            Ast::Let(_) => write!(f, "let"),
            Ast::Set(_) => write!(f, "set"),
            Ast::Nil => todo!(),
            Ast::Atom(_) => todo!(),
            Ast::Integer(_) => todo!(),
            Ast::Float(_) => todo!(),
            Ast::String(_) => todo!(),
            Ast::Boolean(_) => todo!(),
            Ast::List(_) => todo!(),
        }
    }
}

fn eval_quasiquote(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Value> {
    match ast {
        Ast::List(mut list) => {
            if !list.is_empty() {
                if let Ast::Atom(t) = &list[0] {
                    if t.source() == "unquote" {
                        if list.len() != 2 {
                            anyhow::bail!("{}: unquote takes exactly 1 argument", t.loc);
                        }
                        return eval(list.pop().unwrap(), env);
                    } else if t.source() == "unquote-splicing" {
                        anyhow::bail!(
                            "{}: unquote-splicing invalid at top level of quasiquote",
                            t.loc
                        );
                    }
                }
            }
            let mut result = Vec::new();
            for item in list {
                if let Ast::List(mut sublist) = item.clone() {
                    if !sublist.is_empty() {
                        if let Ast::Atom(t) = sublist[0].clone() {
                            if t.source() == "unquote-splicing" {
                                if sublist.len() != 2 {
                                    anyhow::bail!(
                                        "{}: unquote-splicing takes exactly 1 argument",
                                        t.loc
                                    );
                                }
                                let val = eval(sublist.pop().unwrap(), env.clone())?;
                                match val {
                                    Value::List(items) => result.extend(items),
                                    Value::Nil => {}
                                    _ => {
                                        anyhow::bail!("{}: unquote-splicing requires a list", t.loc)
                                    }
                                }
                                continue;
                            }
                        }
                    }
                }
                result.push(eval_quasiquote(item, env.clone())?);
            }
            Ok(Value::List(result))
        }
        _ => Ok(ast_to_value(ast)),
    }
}

fn eval(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Value> {
    match ast {
        Ast::Atom(token) => {
            if let Some(v) = env.borrow().get(token.source()) {
                Ok(v.clone())
            } else {
                anyhow::bail!("{}: Undefined bind `{}`", token.loc, token.source());
            }
        }
        Ast::Define(loc) | Ast::Let(loc) | Ast::Set(loc) => {
            anyhow::bail!("{}: Unexpected `{}`.", loc, ast);
        }
        Ast::Nil => Ok(Value::Nil),
        Ast::Integer(i) => Ok(Value::Integer(i)),
        Ast::Float(f) => Ok(Value::Float(f)),
        Ast::String(s) => Ok(Value::String(s)),
        Ast::Boolean(b) => Ok(Value::Boolean(b)),
        Ast::List(asts) => {
            if asts.is_empty() {
                return Ok(Value::Nil);
            }
            let mut iter = asts.into_iter();
            let first = iter.next().unwrap();

            if let Ast::Define(_) = first {
                let name_ast = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Expected identifier in define"))?;
                let expr_ast = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Expected expression in define"))?;
                let Ast::Atom(name) = name_ast else {
                    anyhow::bail!("Expected identifier in define");
                };
                let val = eval(expr_ast, env.clone())?;
                env.borrow_mut().insert(name.source, val.clone());
                return Ok(val);
            }

            if let Ast::Let(_) = first {
                let bindings_ast = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Expected bindings in let"))?;
                let mut let_env = Env::new(Some(env.clone()));
                match bindings_ast {
                    Ast::List(bindings) => {
                        for bind in bindings {
                            if let Ast::List(mut pair) = bind {
                                if pair.len() != 2 {
                                    anyhow::bail!("Invalid binding pair in let");
                                }
                                let val_ast = pair.pop().unwrap();
                                let name_ast = pair.pop().unwrap();
                                if let Ast::Atom(t) = name_ast {
                                    let val = eval(val_ast, env.clone())?;
                                    let_env.insert(t.source, val);
                                } else {
                                    anyhow::bail!("Expected identifier in let binding");
                                }
                            } else {
                                anyhow::bail!("Expected binding pair in let");
                            }
                        }
                    }
                    Ast::Nil => {} // no bindings
                    _ => anyhow::bail!("Expected bindings list in let"),
                }

                let let_env_rc = Rc::new(RefCell::new(let_env));
                let mut last_val = Value::Nil;
                let mut has_body = false;
                for ast in iter {
                    has_body = true;
                    last_val = eval(ast, let_env_rc.clone())?;
                }
                if !has_body {
                    anyhow::bail!("Expected body in let");
                }
                return Ok(last_val);
            }

            if let Ast::Set(_) = first {
                let name_ast = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Expected identifier in set!"))?;
                let expr_ast = iter
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Expected expression in set!"))?;
                let Ast::Atom(name) = name_ast else {
                    anyhow::bail!("Expected identifier in set!");
                };
                let val = eval(expr_ast, env.clone())?;

                if !env.borrow_mut().set(&name.source, val.clone()) {
                    anyhow::bail!("Unbound variable in set!: {}", name.source);
                }
                return Ok(val);
            }

            if let Ast::Atom(token) = &first {
                match token.source() {
                    "lambda" => {
                        let params_ast = iter
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Expected parameters in lambda"))?;
                        let mut params = Vec::new();
                        match params_ast {
                            Ast::List(p) => {
                                for param in p {
                                    if let Ast::Atom(t) = param {
                                        params.push(t.source);
                                    } else {
                                        anyhow::bail!("Expected identifier in lambda parameters");
                                    }
                                }
                            }
                            Ast::Nil => {} // No params
                            _ => anyhow::bail!("Expected parameter list in lambda"),
                        }

                        let mut body_asts = Vec::new();
                        for ast in iter {
                            body_asts.push(ast);
                        }
                        if body_asts.is_empty() {
                            anyhow::bail!("Expected body in lambda");
                        }
                        let body = if body_asts.len() == 1 {
                            body_asts.pop().unwrap()
                        } else {
                            let mut begin_list = vec![Ast::Atom(Token {
                                kind: TokenKind::Identifier,
                                loc: token.loc.clone(),
                                source: "begin".to_string(),
                            })];
                            begin_list.extend(body_asts);
                            Ast::List(begin_list)
                        };

                        return Ok(Value::Closure {
                            params,
                            body,
                            env: env.clone(),
                        });
                    }
                    "if" => {
                        let cond_ast = iter
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Missing condition in if"))?;
                        let true_ast = iter
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Missing true branch in if"))?;
                        let false_ast = iter.next();

                        let cond = eval(cond_ast, env.clone())?;
                        let is_true = match cond {
                            Value::Boolean(false) => false,
                            _ => true,
                        };
                        if is_true {
                            return eval(true_ast, env);
                        } else if let Some(false_ast) = false_ast {
                            return eval(false_ast, env);
                        } else {
                            return Ok(Value::Nil);
                        }
                    }
                    "quote" => {
                        let expr = iter
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Expected expression in quote"))?;
                        return Ok(ast_to_value(expr));
                    }
                    "quasiquote" => {
                        let expr = iter
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Expected expression in quasiquote"))?;
                        return eval_quasiquote(expr, env);
                    }
                    "begin" => {
                        let mut last_val = Value::Nil;
                        for ast in iter {
                            last_val = eval(ast, env.clone())?;
                        }
                        return Ok(last_val);
                    }
                    "and" => {
                        let mut last_val = Value::Boolean(true);
                        for ast in iter {
                            last_val = eval(ast, env.clone())?;
                            if let Value::Boolean(false) = last_val {
                                return Ok(Value::Boolean(false));
                            }
                        }
                        return Ok(last_val);
                    }
                    "or" => {
                        let mut last_val = Value::Boolean(false);
                        for ast in iter {
                            last_val = eval(ast, env.clone())?;
                            if !matches!(last_val, Value::Boolean(false)) {
                                return Ok(last_val);
                            }
                        }
                        return Ok(last_val);
                    }
                    _ => {} // Fallthrough
                }
            }

            let func_val = eval(first, env.clone())?;
            let mut args = Vec::new();
            for arg_ast in iter {
                args.push(eval(arg_ast, env.clone())?);
            }

            match func_val {
                Value::NativeFunction(f) => f(args, env),
                Value::Closure {
                    params,
                    body,
                    env: closure_env,
                } => {
                    if params.len() != args.len() {
                        anyhow::bail!(
                            "Arity mismatch: expected {} args, got {}",
                            params.len(),
                            args.len()
                        );
                    }
                    let mut call_env = Env::new(Some(closure_env.clone()));
                    for (param, arg) in params.into_iter().zip(args) {
                        call_env.insert(param, arg);
                    }
                    eval(body, Rc::new(RefCell::new(call_env)))
                }
                _ => anyhow::bail!("Attempt to call non-function value"),
            }
        }
    }
}

fn format_value_internal(val: &Value, display: bool) -> String {
    match val {
        Value::Nil => "nil".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => {
            if display {
                s.clone()
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Boolean(b) => (if *b { "#t" } else { "#f" }).to_string(),
        Value::Symbol(s) => s.clone(),
        Value::NativeFunction(_) => "<native-function>".to_string(),
        Value::Closure { .. } => "<closure>".to_string(),
        Value::List(l) => {
            let items: Vec<String> = l
                .iter()
                .map(|v| format_value_internal(v, display))
                .collect();
            format!("({})", items.join(" "))
        }
    }
}

fn format_value(val: &Value) -> String {
    format_value_internal(val, false)
}

fn display_value(val: &Value) -> String {
    format_value_internal(val, true)
}

fn print(val: Value) -> Result<()> {
    println!("{}", format_value(&val));
    Ok(())
}

mod sel_core {
    use anyhow::Result;
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::{Env, Value};

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
                _ => anyhow::bail!("Invalid argument to +: expected number"),
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
            anyhow::bail!("Expected at least 1 argument to -");
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
            _ => anyhow::bail!("Invalid argument to -: expected number"),
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
                _ => anyhow::bail!("Invalid argument to -: expected number"),
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
                _ => anyhow::bail!("Invalid argument to *: expected number"),
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
            anyhow::bail!("Expected at least 1 argument to /");
        }

        if args.len() == 1 {
            match args[0] {
                Value::Integer(i) => return Ok(Value::Float(1.0 / i as f64)),
                Value::Float(f) => return Ok(Value::Float(1.0 / f)),
                _ => anyhow::bail!("Invalid argument to /: expected number"),
            }
        }

        let mut float_val = match args[0] {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => anyhow::bail!("Invalid argument to /: expected number"),
        };

        for arg in args.into_iter().skip(1) {
            match arg {
                Value::Integer(i) => float_val /= i as f64,
                Value::Float(f) => float_val /= f,
                _ => anyhow::bail!("Invalid argument to /: expected number"),
            }
        }
        Ok(Value::Float(float_val))
    }

    pub fn modulo(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            anyhow::bail!("modulo requires exactly 2 arguments");
        }
        let a = match args[0] {
            Value::Integer(i) => i,
            _ => anyhow::bail!("modulo requires integer"),
        };
        let b = match args[1] {
            Value::Integer(i) => i,
            _ => anyhow::bail!("modulo requires integer"),
        };
        Ok(Value::Integer(a % b))
    }

    fn compare_nums(args: Vec<Value>, op: fn(f64, f64) -> bool) -> Result<Value> {
        if args.len() < 2 {
            anyhow::bail!("comparison requires at least 2 arguments");
        }
        let mut prev = match args[0] {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => anyhow::bail!("comparison requires numbers"),
        };
        for arg in args.into_iter().skip(1) {
            let curr = match arg {
                Value::Integer(i) => i as f64,
                Value::Float(f) => f,
                _ => anyhow::bail!("comparison requires numbers"),
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
            anyhow::bail!("cons requires exactly 2 arguments");
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
            anyhow::bail!("car requires exactly 1 argument");
        }
        match args.pop().unwrap() {
            Value::List(mut l) => {
                if l.is_empty() {
                    anyhow::bail!("car on empty list");
                }
                Ok(l.remove(0))
            }
            _ => anyhow::bail!("car requires a list"),
        }
    }

    pub fn nth(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 2 {
            anyhow::bail!("nth requires exactly 2 argument");
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
                _ => anyhow::bail!("nth requires a interger"),
            },
            _ => anyhow::bail!("nth requires a list"),
        }
    }

    pub fn cdr(mut args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("cdr requires exactly 1 argument");
        }
        match args.pop().unwrap() {
            Value::List(mut l) => {
                if l.is_empty() {
                    anyhow::bail!("cdr on empty list");
                }
                l.remove(0);
                Ok(if l.is_empty() {
                    Value::Nil
                } else {
                    Value::List(l)
                })
            }
            _ => anyhow::bail!("cdr requires a list"),
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
            anyhow::bail!("nil? requires exactly 1 argument");
        }
        match &args[0] {
            Value::Nil => Ok(Value::Boolean(true)),
            Value::List(l) if l.is_empty() => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn is_list(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("islist requires exactly 1 argument");
        }
        match &args[0] {
            Value::List(l) if !l.is_empty() => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn is_number(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("isnumber requires exactly 1 argument");
        }
        Ok(Value::Boolean(matches!(
            args[0],
            Value::Integer(_) | Value::Float(_)
        )))
    }

    pub fn is_string(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("isstring requires exactly 1 argument");
        }
        Ok(Value::Boolean(matches!(args[0], Value::String(_))))
    }

    pub fn is_function(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("isfunction requires exactly 1 argument");
        }
        Ok(Value::Boolean(matches!(
            args[0],
            Value::NativeFunction(_) | Value::Closure { .. }
        )))
    }

    pub fn not(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("not requires exactly 1 argument");
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

    pub fn display_func(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        for arg in args {
            print!("{} ", crate::display_value(&arg));
        }
        println!();
        Ok(Value::Nil)
    }

    pub fn load(env: Rc<RefCell<Env>>) {
        let mut e = env.borrow_mut();
        e.insert(String::from("+"), Value::NativeFunction(sum));
        e.insert(String::from("-"), Value::NativeFunction(sub));
        e.insert(String::from("*"), Value::NativeFunction(mul));
        e.insert(String::from("/"), Value::NativeFunction(div));
        e.insert(String::from("mod"), Value::NativeFunction(modulo));

        e.insert(String::from("="), Value::NativeFunction(num_eq));
        e.insert(String::from("<"), Value::NativeFunction(num_lt));
        e.insert(String::from(">"), Value::NativeFunction(num_gt));
        e.insert(String::from("<="), Value::NativeFunction(num_lte));
        e.insert(String::from(">="), Value::NativeFunction(num_gte));

        e.insert(String::from("cons"), Value::NativeFunction(cons));
        e.insert(String::from("car"), Value::NativeFunction(car));
        e.insert(String::from("first"), Value::NativeFunction(car));
        e.insert(String::from("cdr"), Value::NativeFunction(cdr));
        e.insert(String::from("rest"), Value::NativeFunction(cdr));
        e.insert(String::from("nth"), Value::NativeFunction(nth));
        e.insert(String::from("list"), Value::NativeFunction(list));

        e.insert(String::from("nil?"), Value::NativeFunction(is_nil));
        e.insert(String::from("list?"), Value::NativeFunction(is_list));
        e.insert(String::from("number?"), Value::NativeFunction(is_number));
        e.insert(String::from("string?"), Value::NativeFunction(is_string));
        e.insert(
            String::from("function?"),
            Value::NativeFunction(is_function),
        );

        e.insert(String::from("not"), Value::NativeFunction(not));
        e.insert(String::from("print"), Value::NativeFunction(print_func));
        e.insert(String::from("display"), Value::NativeFunction(display_func));
    }
}
