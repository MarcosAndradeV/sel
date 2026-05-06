use anyhow::Result;
use lex_just_parse::lexer::*;
use lex_just_parse::parser::*;
use lex_just_parse::try_parse;

use std::collections::HashMap;
use std::env::args;
use std::fs;
use std::io::Write as _;
use std::io::stdin;
use std::io::stdout;

use std::cell::RefCell;
use std::rc::Rc;

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
    let mut args = args().skip(1).rev();
    if let Some(arg) = args.next() {
        let src = fs::read_to_string(arg)?;
        let program = format!("(begin {src})");
        let ast = read(&program)?;
        eval(ast, env).map(|_|())
    } else {
        println!("Welcome to the Sel Scheme repl. (Use `quit` to exit)");
        repl("sel> ", env)
    }
}

fn repl(prompt: &str, env: Rc<RefCell<Env>>) -> Result<()> {
    let mut line_buffer = String::new();
    loop {
        print!("{prompt}");
        stdout().flush()?;
        stdin().read_line(&mut line_buffer)?;
        let line = line_buffer.trim();

        match line {
            "" => continue,
            "quit" => break,
            _ => (),
        }

        let ast = read(line)?;
        let val = eval(ast, env.clone())?;
        print(val)?;

        line_buffer.clear();
    }
    Ok(())
}

fn read(line: &str) -> Result<Ast> {
    let mut lex = Lexer::new(line);
    parse_ast(&mut lex)
        .success()
        .map_err(|(lex, error)| anyhow::anyhow!("{}: {}", lex.peek().loc, error.to_string()))
}

fn parse_ast<'lex>(lex: RefLexer) -> Parser<Ast, anyhow::Error> {
    let ptoken = lex.peek();
    match ptoken.kind {
        TokenKind::OpenParen => {
            let (lex, seq) = try_parse!(parse_sequence(
                lex,
                TokenKind::OpenParen,
                TokenKind::CloseParen,
                None,
                parse_ast
            ));
            return Parser::Success(lex, Ast::List(seq));
        }
        TokenKind::Int(base) => {
            let token = lex.next();
            return Parser::Success(
                lex,
                Ast::Integer(i64::from_str_radix(token.source(), base.radix()).unwrap()),
            );
        }
        TokenKind::RealNumber => {
            let token = lex.next();
            return Parser::Success(lex, Ast::Float(token.source().parse().unwrap()));
        }
        TokenKind::StringLiteral => {
            let token = lex.next();
            return Parser::Success(lex, Ast::String(token.unescape()));
        }
        TokenKind::Directive if ptoken.source() == "#t" => {
            lex.next();
            return Parser::Success(lex, Ast::Boolean(true));
        }
        TokenKind::Directive if ptoken.source() == "#f" => {
            lex.next();
            return Parser::Success(lex, Ast::Boolean(false));
        }
        TokenKind::Identifier if ptoken.source() == "nil" => {
            lex.next();
            return Parser::Success(lex, Ast::Nil);
        }
        TokenKind::Identifier if ptoken.source() == "define" => {
            let loc = lex.next().loc;
            return Parser::Success(lex, Ast::Define(loc));
        }
        TokenKind::Identifier if ptoken.source() == "let" => {
            let loc = lex.next().loc;
            return Parser::Success(lex, Ast::Let(loc));
        }
        TokenKind::Identifier if ptoken.source() == "set" => {
            let token = lex.next();
            if lex.peek().kind == TokenKind::Bang {
                lex.next();
                return Parser::Success(lex, Ast::Set(token.loc));
            } else {
                return Parser::Success(lex, Ast::Atom(token));
            }
        }
        _ => {
            let token = lex.next();
            return Parser::Success(lex, Ast::Atom(token));
        }
    }
}

fn parse_sequence<'lex, F, T>(
    mut lex: RefLexer,
    start: TokenKind,
    stop: TokenKind,
    separator: Option<TokenKind>,
    mut parser_fn: F,
) -> Parser<Vec<T>, anyhow::Error>
where
    F: FnMut(RefLexer) -> Parser<T, anyhow::Error>,
{
    let mut sequence = Vec::new();
    let token = lex.next();
    if token.kind != start {
        return Parser::Fail(
            lex,
            anyhow::anyhow!("expected {:?}, found {:?}", start, token.kind),
        );
    }
    loop {
        let token = lex.peek();
        if token.kind == stop {
            lex.next();
            break;
        }
        let (l, expr) = try_parse!(parser_fn(lex));
        lex = l;
        sequence.push(expr);
        if separator.is_some_and(|sep| sep == lex.peek().kind) {
            lex.next();
        }
    }
    Parser::Success(lex, sequence)
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
            Ast::Set(_) => write!(f, "set!"),
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
                            let mut begin_list = vec![Ast::Atom(Token::new(
                                lex_just_parse::lexer::TokenKind::Identifier,
                                token.loc,
                                "begin".to_string(),
                            ))];
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

fn format_value(val: &Value) -> String {
    match val {
        Value::Nil => "nil".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Boolean(b) => (if *b { "#t" } else { "#f" }).to_string(),
        Value::Symbol(s) => s.clone(),
        Value::NativeFunction(_) => "<native-function>".to_string(),
        Value::Closure { .. } => "<closure>".to_string(),
        Value::List(l) => {
            let items: Vec<String> = l.iter().map(format_value).collect();
            format!("({})", items.join(" "))
        }
    }
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

    pub fn is_null(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("isnull requires exactly 1 argument");
        }
        match &args[0] {
            Value::Nil => Ok(Value::Boolean(true)),
            Value::List(l) if l.is_empty() => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(false)),
        }
    }

    pub fn is_pair(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("ispair requires exactly 1 argument");
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

    pub fn is_procedure(args: Vec<Value>, _env: Rc<RefCell<Env>>) -> Result<Value> {
        if args.len() != 1 {
            anyhow::bail!("isprocedure requires exactly 1 argument");
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
        e.insert(String::from("cdr"), Value::NativeFunction(cdr));
        e.insert(String::from("list"), Value::NativeFunction(list));

        e.insert(String::from("isnull"), Value::NativeFunction(is_null));
        e.insert(String::from("ispair"), Value::NativeFunction(is_pair));
        e.insert(String::from("isnumber"), Value::NativeFunction(is_number));
        e.insert(String::from("isstring"), Value::NativeFunction(is_string));
        e.insert(
            String::from("isprocedure"),
            Value::NativeFunction(is_procedure),
        );

        e.insert(String::from("not"), Value::NativeFunction(not));
    }
}
