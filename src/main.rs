use anyhow::Result;
use lex_just_parse::lexer::*;
use lex_just_parse::parser::*;
use lex_just_parse::try_parse;

use std::collections::HashMap;
use std::io::Write as _;
use std::io::stdin;
use std::io::stdout;

use std::rc::Rc;
use std::cell::RefCell;

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
}

fn main() -> Result<()> {
    let env = Rc::new(RefCell::new(Env::default()));
    sel_core::load(env.clone());
    println!("Welcome to the Sel Scheme repl. (Use `quit` to exit)");
    repl("sel> ", env)
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
        .map_err(|(lex, error)| anyhow::anyhow!("repl:{}: {}", lex.peek().loc, error.to_string()))
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
        TokenKind::Identifier if ptoken.source() == "nil" => {
            lex.next();
            return Parser::Success(lex, Ast::Nil);
        }
        TokenKind::Identifier if ptoken.source() == "define" => {
            let loc = lex.next().loc;
            return Parser::Success(lex, Ast::Define(loc));
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
    NativeFunction(fn(Vec<Value>) -> Result<Value>),
}

#[derive(Debug)]
enum Ast {
    Define(Loc),
    Nil,
    Atom(Token),
    Integer(i64),
    List(Vec<Self>),
}

fn eval(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Value> {
    match ast {
        Ast::Atom(token) => {
            if let Some(v) = env.borrow().get(token.source()) {
                Ok(v.clone())
            } else {
                anyhow::bail!("repl:{}: Undefined bind `{}`", token.loc, token.source());
            }
        }
        Ast::Define(loc) => {
            anyhow::bail!(
                "repl:{}: Unexpected `define`. Hint: Try `(define x 10)`",
                loc
            );
        }
        Ast::Nil => Ok(Value::Nil),
        Ast::Integer(i) => Ok(Value::Integer(i)),
        Ast::List(mut asts) => match asts.as_slice() {
            [Ast::Define(_), Ast::Atom(_), _] => {
                let expr = asts.pop().unwrap();
                let Ast::Atom(name) = asts.pop().unwrap() else {
                    unreachable!()
                };
                let val = eval(expr, env.clone())?;
                env.borrow_mut().insert(name.source, val.clone());
                Ok(val)
            }
            [Ast::Atom(_), ..] => {
                let Ast::Atom(token) = asts.remove(0) else {
                    unreachable!()
                };
                let mut args = Vec::with_capacity(asts.len());
                for arg in asts {
                    args.push(eval(arg, env.clone())?);
                }
                let func = env.borrow().get(token.source());
                if let Some(Value::NativeFunction(f)) = func {
                    f(args)
                } else {
                    anyhow::bail!("repl:{}: Undefined bind `{}`", token.loc, token.source());
                }
            }
            [] => Ok(Value::Nil),
            _ => anyhow::bail!("Invalid list form"),
        },
    }
}

fn print(val: Value) -> Result<()> {
    match val {
        Value::Nil => println!("nil"),
        Value::Integer(i) => println!("{i}"),
        Value::NativeFunction(_) => println!("<native-function>"),
    }
    Ok(())
}

mod sel_core {
    use anyhow::Result;
    use std::rc::Rc;
    use std::cell::RefCell;

    use crate::{Env, Value};

    pub fn sum(args: Vec<Value>) -> Result<Value> {
        let mut sum = 0;
        for arg in args {
            match arg {
                Value::Integer(i) => sum += i,
                _ => anyhow::bail!("Invalid argument to +: expected Integer"),
            }
        }
        Ok(Value::Integer(sum))
    }

    pub fn sub(args: Vec<Value>) -> Result<Value> {
        let mut sum = 0;
        for arg in args {
            match arg {
                Value::Integer(i) => sum -= i,
                _ => anyhow::bail!("Invalid argument to -: expected Integer"),
            }
        }
        Ok(Value::Integer(sum))
    }

    pub fn load(env: Rc<RefCell<Env>>) {
        env.borrow_mut().insert(String::from("+"), Value::NativeFunction(sum));
        env.borrow_mut().insert(String::from("-"), Value::NativeFunction(sub));
    }
}
