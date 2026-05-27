use crate::diagnostics::*;
use crate::lexer::*;
use crate::parser::optimize_ast;
use crate::types::Record;
use crate::types::intern;
use crate::types::lookup;
use crate::value::Value;
use std::rc::Rc;

type Result<T> = std::result::Result<T, SelError>;

#[derive(Debug, Clone)]
pub enum Ast {
    Define(Loc, u32, Box<Ast>),
    DefMacro(Loc, u32, Box<Ast>),
    Import(Loc, u32, Option<u32>),
    Let(Loc, Vec<(u32, Ast)>, Vec<Ast>),
    Set(Loc, u32, Box<Ast>),
    If(Loc, Box<Ast>, Box<Ast>, Option<Box<Ast>>),
    Unless(Loc, Box<Ast>, Box<Ast>, Option<Box<Ast>>),
    When(Loc, Box<Ast>, Vec<Ast>),
    Cond(Loc, Vec<(Ast, Ast)>),
    While(Loc, Box<Ast>, Box<Ast>),
    Until(Loc, Box<Ast>, Box<Ast>),
    Lambda(Loc, Vec<u32>, Vec<Ast>),
    Begin(Loc, Vec<Ast>),
    Quote(Loc, Box<Ast>),
    Quasiquote(Loc, Box<Ast>),
    Unquote(Loc, Box<Ast>),
    UnquoteSplicing(Loc, Box<Ast>),
    And(Loc, Vec<Ast>),
    Or(Loc, Vec<Ast>),
    Bind(Loc, u32),
    Nil(Loc),
    Symbol(Loc, u32),
    Integer(Loc, i64),
    Float(Loc, f64),
    String(Loc, String),
    Boolean(Loc, bool),
    List(Loc, Vec<Self>),
    Record(Loc, Vec<(u32, Self)>),
    Try(Loc, Box<Ast>, u32, Vec<Ast>),
    Yield(Loc, Box<Ast>),
    CoResume(Loc, Box<Ast>, Box<Ast>),
    Char(Loc, char),
    VisibilityDirective(Loc, bool),
}

impl Ast {
    pub fn loc(&self) -> Loc {
        match self {
            Ast::Define(loc, ..) => *loc,
            Ast::DefMacro(loc, ..) => *loc,
            Ast::Import(loc, ..) => *loc,
            Ast::Let(loc, ..) => *loc,
            Ast::Set(loc, ..) => *loc,
            Ast::If(loc, ..) => *loc,
            Ast::Unless(loc, ..) => *loc,
            Ast::When(loc, ..) => *loc,
            Ast::Cond(loc, ..) => *loc,
            Ast::While(loc, ..) => *loc,
            Ast::Until(loc, ..) => *loc,
            Ast::Lambda(loc, ..) => *loc,
            Ast::Begin(loc, ..) => *loc,
            Ast::Quote(loc, ..) => *loc,
            Ast::Quasiquote(loc, ..) => *loc,
            Ast::Unquote(loc, ..) => *loc,
            Ast::UnquoteSplicing(loc, ..) => *loc,
            Ast::And(loc, ..) => *loc,
            Ast::Or(loc, ..) => *loc,
            Ast::Nil(loc, ..) => *loc,
            Ast::Symbol(loc, ..) => *loc,
            Ast::Integer(loc, ..) => *loc,
            Ast::Float(loc, ..) => *loc,
            Ast::String(loc, ..) => *loc,
            Ast::Boolean(loc, ..) => *loc,
            Ast::List(loc, ..) => *loc,
            Ast::Bind(loc, ..) => *loc,
            Ast::Record(loc, ..) => *loc,
            Ast::Try(loc, ..) => *loc,
            Ast::Yield(loc, ..) => *loc,
            Ast::CoResume(loc, ..) => *loc,
            Ast::Char(loc, ..) => *loc,
            Ast::VisibilityDirective(loc, ..) => *loc,
        }
    }
}

impl std::fmt::Display for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ast::Define(..) => write!(f, "define"),
            Ast::DefMacro(..) => write!(f, "defmacro"),
            Ast::Import(..) => write!(f, "import"),
            Ast::Let(..) => write!(f, "let"),
            Ast::Set(..) => write!(f, "set"),
            Ast::If(..) => write!(f, "if"),
            Ast::Cond(..) => write!(f, "cond"),
            Ast::When(..) => write!(f, "when"),
            Ast::Unless(..) => write!(f, "unless"),
            Ast::While(..) => write!(f, "while"),
            Ast::Until(..) => write!(f, "while"),
            Ast::Lambda(..) => write!(f, "lambda"),
            Ast::Begin(..) => write!(f, "begin"),
            Ast::Quote(..) => write!(f, "quote"),
            Ast::Quasiquote(..) => write!(f, "quasiquote"),
            Ast::Unquote(..) => write!(f, "unquote"),
            Ast::UnquoteSplicing(..) => write!(f, "unquote-splicing"),
            Ast::And(..) => write!(f, "and"),
            Ast::Or(..) => write!(f, "or"),
            Ast::Nil(_) => write!(f, "nil"),
            Ast::Symbol(_, id) => write!(f, "{}", lookup(*id)),
            Ast::Integer(_, i) => write!(f, "{i}"),
            Ast::Float(_, n) => write!(f, "{n}"),
            Ast::String(_, s) => write!(f, "\"{s}\""),
            Ast::Boolean(_, b) => write!(f, "{}", if *b { "#t" } else { "#f" }),
            Ast::List(..) => write!(f, "<list>"),
            Ast::Record(..) => write!(f, "<record>"),
            Ast::Bind(_, id) => write!(f, "&{}", lookup(*id)),
            Ast::Try(..) => write!(f, "try"),
            Ast::Yield(..) => write!(f, "co-yield"),
            Ast::CoResume(..) => write!(f, "co-resume"),
            Ast::Char(_, c) => match c {
                ' ' => write!(f, "#\\space"),
                '\n' => write!(f, "#\\newline"),
                '\t' => write!(f, "#\\tab"),
                '\r' => write!(f, "#\\return"),
                ch => write!(f, "#\\{}", ch),
            },
            Ast::VisibilityDirective(_, is_public) => {
                write!(f, "{}", if *is_public { ":public" } else { ":private" })
            }
        }
    }
}

pub fn ast_to_value(ast: Ast) -> (Loc, Value) {
    match ast {
        Ast::Symbol(loc, id) => (loc, Value::Symbol(id)),
        Ast::Integer(loc, i) => (loc, Value::Integer(i)),
        Ast::Float(loc, f) => (loc, Value::Float(f)),
        Ast::String(loc, s) => (loc, Value::String(Rc::new(s))),
        Ast::Boolean(loc, b) => (loc, Value::Boolean(b)),
        Ast::Nil(loc) => (loc, Value::Nil),
        Ast::List(loc, l) => (
            loc,
            Value::List(
                l.into_iter()
                    .map(|a| ast_to_value(a).1)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
                    .into(),
            ),
        ),
        Ast::Define(loc, id, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("define")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::DefMacro(loc, id, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("defmacro")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::Import(loc, id, alias) => {
            let mut list = vec![Value::Symbol(intern("import")), Value::Symbol(id)];
            if let Some(a) = alias {
                list.push(Value::Symbol(intern(":as")));
                list.push(Value::Symbol(a));
            }
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Set(loc, id, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("set!")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::If(loc, cond, t, f) => {
            let mut list = vec![
                Value::Symbol(intern("if")),
                ast_to_value(*cond).1,
                ast_to_value(*t).1,
            ];
            if let Some(f) = f {
                list.push(ast_to_value(*f).1);
            }
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Cond(loc, branches) => {
            let mut list = vec![Value::Symbol(intern("cond"))];
            for (c, e) in branches {
                list.push(ast_to_value(c).1);
                list.push(ast_to_value(e).1);
            }
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::When(loc, cond, body) => {
            let mut list = vec![
                Value::Symbol(intern("when")),
                ast_to_value(*cond).1,
            ];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Unless(loc, cond, f, t) => {
            let mut list = vec![
                Value::Symbol(intern("unless")),
                ast_to_value(*cond).1,
                ast_to_value(*f).1,
            ];
            if let Some(t) = t {
                list.push(ast_to_value(*t).1);
            }
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::While(loc, cond, body) => {
            let list = vec![
                Value::Symbol(intern("while")),
                ast_to_value(*cond).1,
                ast_to_value(*body).1,
            ];
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Until(loc, cond, body) => {
            let list = vec![
                Value::Symbol(intern("until")),
                ast_to_value(*cond).1,
                ast_to_value(*body).1,
            ];
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Lambda(loc, params, body) => {
            let mut list = vec![
                Value::Symbol(intern("lambda")),
                Value::List(
                    params
                        .into_iter()
                        .map(Value::Symbol)
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                        .into(),
                ),
            ];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Begin(loc, body) => {
            let mut list = vec![Value::Symbol(intern("begin"))];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Let(loc, bindings, body) => {
            let mut list = vec![Value::Symbol(intern("let"))];
            let mut bind_list = Vec::new();
            for (id, val) in bindings {
                bind_list.push(Value::List(Rc::new([
                    Value::Symbol(id),
                    ast_to_value(val).1,
                ])));
            }
            list.push(Value::List(bind_list.into_boxed_slice().into()));
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Quote(loc, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("quote")),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::Quasiquote(loc, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("quasiquote")),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::Unquote(loc, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("unquote")),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::UnquoteSplicing(loc, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("unquote-splicing")),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::And(loc, exprs) => {
            let mut list = vec![Value::Symbol(intern("and"))];
            list.extend(exprs.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Or(loc, exprs) => {
            let mut list = vec![Value::Symbol(intern("or"))];
            list.extend(exprs.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Bind(loc, id) => (loc, Value::Symbol(intern(&format!("&{}", lookup(id))))),
        Ast::Try(loc, body, err_var, catch_body) => {
            let list = vec![
                Value::Symbol(intern("try")),
                ast_to_value(*body).1,
                Value::List({
                    let mut catch_list =
                        vec![Value::Symbol(intern("catch")), Value::Symbol(err_var)];
                    catch_list.extend(catch_body.into_iter().map(|a| ast_to_value(a).1));
                    catch_list.into_boxed_slice().into()
                }),
            ];
            (loc, Value::List(list.into_boxed_slice().into()))
        }
        Ast::Yield(loc, val) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("co-yield")),
                ast_to_value(*val).1,
            ])),
        ),
        Ast::CoResume(loc, co, arg) => (
            loc,
            Value::List(Rc::new([
                Value::Symbol(intern("co-resume")),
                ast_to_value(*co).1,
                ast_to_value(*arg).1,
            ])),
        ),
        Ast::Char(loc, c) => (loc, Value::Char(c)),
        Ast::VisibilityDirective(loc, is_public) => (
            loc,
            Value::Symbol(intern(if is_public { ":public" } else { ":private" })),
        ),
        Ast::Record(loc, record) => (
            loc,
            Value::Record(Rc::new(Record::new().populate(
                record.into_iter().map(|(k, ast)| (k, ast_to_value(ast).1)),
            ))),
        ),
    }
}

pub fn value_to_ast(val: Value, loc: Loc) -> Result<Ast> {
    match val {
        Value::Nil => Ok(Ast::Nil(loc)),
        Value::Integer(i) => Ok(Ast::Integer(loc, i)),
        Value::Float(f) => Ok(Ast::Float(loc, f)),
        Value::String(s) => Ok(Ast::String(loc, (*s).clone())),
        Value::Boolean(b) => Ok(Ast::Boolean(loc, b)),
        Value::Symbol(id) => Ok(Ast::Symbol(loc, id)),
        Value::Char(c) => Ok(Ast::Char(loc, c)),
        Value::List(l) => {
            let mut ast_list = Vec::new();
            for v in l.iter() {
                ast_list.push(value_to_ast(v.clone(), loc)?);
            }
            // Need to re-run parse_list logic to get optimized AST
            // Or we could just return Ast::List and let eval handle it
            // but we want optimized AST.
            // Let's use a helper that simulates the parser's logic.
            if ast_list.is_empty() {
                return Ok(Ast::Nil(loc));
            }
            optimize_ast(ast_list, loc)
        }
        Value::Record(r) => {
            let mut fields = Vec::new();
            for (&k, v) in r.fields().iter() {
                fields.push((k, value_to_ast(v.clone(), loc)?));
            }
            Ok(Ast::Record(loc, fields))
        }
        v => Err(SelError::SyntaxError(
            loc,
            format!("Cannot convert function or macro to AST ({v})"),
        )),
    }
}
