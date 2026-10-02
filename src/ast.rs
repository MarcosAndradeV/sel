use crate::diagnostics::*;
use crate::lexer::*;
use crate::types::Record;
use crate::types::intern;
use crate::types::lookup;
use crate::value::Value;
use std::rc::Rc;

type Result<T> = std::result::Result<T, SelError>;

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard(Loc),
    Variable(Loc, u32),
    Literal(Loc, Box<Ast>),
    List(Loc, Vec<Pattern>),
    Cons(Loc, Box<Pattern>, Box<Pattern>),
    Rest(Loc, Vec<Pattern>, Box<Pattern>),
    Record(Loc, Vec<(u32, Pattern)>),
    Or(Loc, Vec<Pattern>),
    As(Loc, u32, Box<Pattern>),
}

impl Pattern {
    pub fn loc(&self) -> Loc {
        match self {
            Pattern::Wildcard(loc)
            | Pattern::Variable(loc, _)
            | Pattern::Literal(loc, _)
            | Pattern::List(loc, _)
            | Pattern::Cons(loc, _, _)
            | Pattern::Rest(loc, _, _)
            | Pattern::Record(loc, _)
            | Pattern::Or(loc, _)
            | Pattern::As(loc, _, _) => *loc,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MatchClause {
    pub loc: Loc,
    pub pattern: Pattern,
    pub guard: Option<Ast>,
    pub body: Vec<Ast>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineKind {
    ThreadFirst, // |>
    ThreadLast,  // |>>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseTypeKind {
    Int,
    Float,
    Bool,
    String,
    Char,
    Symbol,
    Nil,
    Any,
    Record,
    List,
    Fn,
}

impl std::fmt::Display for BaseTypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaseTypeKind::Int => write!(f, "int"),
            BaseTypeKind::Float => write!(f, "float"),
            BaseTypeKind::Bool => write!(f, "bool"),
            BaseTypeKind::String => write!(f, "string"),
            BaseTypeKind::Char => write!(f, "char"),
            BaseTypeKind::Symbol => write!(f, "symbol"),
            BaseTypeKind::Nil => write!(f, "nil"),
            BaseTypeKind::Any => write!(f, "any"),
            BaseTypeKind::Record => write!(f, "record"),
            BaseTypeKind::List => write!(f, "list"),
            BaseTypeKind::Fn => write!(f, "fn"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Base(Loc, BaseTypeKind),
    Var(Loc, u32),                       // A, B, T
    Nominal(Loc, u32),                   // Foo, Person
    List(Loc, Box<TypeExpr>),            // [A]
    Record(Loc, Vec<(u32, TypeExpr)>),   // { c: int }
    Function(Loc, Vec<TypeExpr>, Box<TypeExpr>), // (A -> bool), [A] -> [A]
    Union(Loc, Vec<TypeExpr>),           // A | B
}

impl TypeExpr {
    pub fn loc(&self) -> Loc {
        match self {
            TypeExpr::Base(loc, _)
            | TypeExpr::Var(loc, _)
            | TypeExpr::Nominal(loc, _)
            | TypeExpr::List(loc, _)
            | TypeExpr::Record(loc, _)
            | TypeExpr::Function(loc, _, _)
            | TypeExpr::Union(loc, _) => *loc,
        }
    }
}

impl std::fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeExpr::Base(_, base) => write!(f, "{}", base),
            TypeExpr::Var(_, var) => write!(f, "{}", lookup(*var)),
            TypeExpr::Nominal(_, name) => write!(f, "{}", lookup(*name)),
            TypeExpr::List(_, elem) => write!(f, "[{}]", elem),
            TypeExpr::Record(_, fields) => {
                write!(f, "{{")?;
                for (i, (key, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", lookup(*key), ty)?;
                }
                write!(f, "}}")
            }
            TypeExpr::Function(_, params, ret) => {
                if params.is_empty() {
                    write!(f, "() -> {}", ret)
                } else {
                    for (i, p) in params.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        if matches!(p, TypeExpr::Function(..)) {
                            write!(f, "({})", p)?;
                        } else {
                            write!(f, "{}", p)?;
                        }
                    }
                    write!(f, " -> {}", ret)
                }
            }
            TypeExpr::Union(_, tys) => {
                for (i, t) in tys.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", t)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitConstraint {
    pub loc: Loc,
    pub type_var: u32,
    pub trait_name: u32, // atom ID e.g. :comparable
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeSignature {
    pub loc: Loc,
    pub forany_vars: Vec<u32>,
    pub constraints: Vec<TraitConstraint>,
    pub fn_type: TypeExpr,
}

#[derive(Debug, Clone)]
pub enum Ast {
    TypeSignature(Loc, u32, TypeSignature),
    TypeAssert(Loc, Box<Ast>, TypeExpr),
    Newtype(Loc, u32, TypeExpr),
    Derive(Loc, u32, u32),
    Implements(Loc, u32, u32, Box<Ast>),
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
    Lambda(Loc, Option<u32>, Vec<u32>, Vec<Ast>),
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
    DotAccess(Loc, Box<Ast>, u32),
    Pipeline(Loc, Box<Ast>, Box<Ast>, PipelineKind),
    Try(Loc, Box<Ast>, u32, Vec<Ast>),
    Yield(Loc, Box<Ast>),
    CoResume(Loc, Box<Ast>, Box<Ast>),
    Char(Loc, char),
    VisibilityDirective(Loc, bool),
    Load(Loc, Box<Ast>),
    Match(Loc, Box<Ast>, Vec<MatchClause>),
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
            Ast::DotAccess(loc, ..) => *loc,
            Ast::Pipeline(loc, ..) => *loc,
            Ast::Try(loc, ..) => *loc,
            Ast::Yield(loc, ..) => *loc,
            Ast::CoResume(loc, ..) => *loc,
            Ast::Char(loc, ..) => *loc,
            Ast::VisibilityDirective(loc, ..) => *loc,
            Ast::Load(loc, ..) => *loc,
            Ast::Match(loc, ..) => *loc,
            Ast::TypeSignature(loc, ..) => *loc,
            Ast::TypeAssert(loc, ..) => *loc,
            Ast::Newtype(loc, ..) => *loc,
            Ast::Derive(loc, ..) => *loc,
            Ast::Implements(loc, ..) => *loc,
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
            Ast::DotAccess(..) => write!(f, "dot-access"),
            Ast::Pipeline(..) => write!(f, "pipeline"),
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
            Ast::Load(..) => write!(f, "load"),
            Ast::Match(..) => write!(f, "match"),
            Ast::TypeSignature(_, name, sig) => write!(f, "{} :: {}", lookup(*name), sig.fn_type),
            Ast::TypeAssert(_, expr, ty) => write!(f, "{} :: {}", expr, ty),
            Ast::Newtype(_, name, ty) => write!(f, "newtype {} := {}", lookup(*name), ty),
            Ast::Derive(_, name, tr) => write!(f, "derive({}, :{})", lookup(*name), lookup(*tr)),
            Ast::Implements(_, name, tr, _) => {
                write!(f, "implements({}, :{}, ...)", lookup(*name), lookup(*tr))
            }
        }
    }
}

pub fn ast_to_value(ast: Ast) -> (Loc, Value) {
    match ast {
        Ast::Load(loc, path) => (
            loc,
            Value::make_list(vec![Value::Symbol(intern("load")), ast_to_value(*path).1]),
        ),
        Ast::Symbol(loc, id) => (loc, Value::Symbol(id)),
        Ast::Integer(loc, i) => (loc, Value::Integer(i)),
        Ast::Float(loc, f) => (loc, Value::Float(f)),
        Ast::String(loc, s) => (loc, Value::make_string(&s)),
        Ast::Boolean(loc, b) => (loc, Value::Boolean(b)),
        Ast::Nil(loc) => (loc, Value::Nil),
        Ast::List(loc, l) => (
            loc,
            Value::make_list(l.into_iter().map(|a| ast_to_value(a).1).collect()),
        ),
        Ast::Define(loc, id, val) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("define")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::DefMacro(loc, id, val) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("defmacro")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::Import(loc, id, alias) => {
            let mut list = vec![Value::Symbol(intern("import")), Value::Symbol(id)];
            if let Some(a) = alias {
                list.push(Value::Symbol(intern(":as")));
                list.push(Value::Symbol(a));
            }
            (loc, Value::make_list(list))
        }
        Ast::Set(loc, id, val) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("set!")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ]),
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
            (loc, Value::make_list(list))
        }
        Ast::Cond(loc, branches) => {
            let mut list = vec![Value::Symbol(intern("cond"))];
            for (c, e) in branches {
                list.push(ast_to_value(c).1);
                list.push(ast_to_value(e).1);
            }
            (loc, Value::make_list(list))
        }
        Ast::When(loc, cond, body) => {
            let mut list = vec![Value::Symbol(intern("when")), ast_to_value(*cond).1];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::make_list(list))
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
            (loc, Value::make_list(list))
        }
        Ast::While(loc, cond, body) => {
            let list = vec![
                Value::Symbol(intern("while")),
                ast_to_value(*cond).1,
                ast_to_value(*body).1,
            ];
            (loc, Value::make_list(list))
        }
        Ast::Until(loc, cond, body) => {
            let list = vec![
                Value::Symbol(intern("until")),
                ast_to_value(*cond).1,
                ast_to_value(*body).1,
            ];
            (loc, Value::make_list(list))
        }
        Ast::Lambda(loc, _name, params, body) => {
            let mut list = vec![
                Value::Symbol(intern("lambda")),
                Value::make_list(params.into_iter().map(Value::Symbol).collect()),
            ];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::make_list(list))
        }
        Ast::Begin(loc, body) => {
            let mut list = vec![Value::Symbol(intern("begin"))];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::make_list(list))
        }
        Ast::Let(loc, bindings, body) => {
            let mut list = vec![Value::Symbol(intern("let"))];
            let mut bind_list = Vec::new();
            for (id, val) in bindings {
                bind_list.push(Value::make_list(vec![
                    Value::Symbol(id),
                    ast_to_value(val).1,
                ]));
            }
            list.push(Value::make_list(bind_list));
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::make_list(list))
        }
        Ast::Quote(loc, val) => (
            loc,
            Value::make_list(vec![Value::Symbol(intern("quote")), ast_to_value(*val).1]),
        ),
        Ast::Quasiquote(loc, val) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("quasiquote")),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::Unquote(loc, val) => (
            loc,
            Value::make_list(vec![Value::Symbol(intern("unquote")), ast_to_value(*val).1]),
        ),
        Ast::UnquoteSplicing(loc, val) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("unquote-splicing")),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::And(loc, exprs) => {
            let mut list = vec![Value::Symbol(intern("and"))];
            list.extend(exprs.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::make_list(list))
        }
        Ast::Or(loc, exprs) => {
            let mut list = vec![Value::Symbol(intern("or"))];
            list.extend(exprs.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::make_list(list))
        }
        Ast::Bind(loc, id) => (loc, Value::Symbol(intern(&format!("&{}", lookup(id))))),
        Ast::Try(loc, body, err_var, catch_body) => {
            let mut catch_list = vec![Value::Symbol(intern("catch")), Value::Symbol(err_var)];
            catch_list.extend(catch_body.into_iter().map(|a| ast_to_value(a).1));
            let list = vec![
                Value::Symbol(intern("try")),
                ast_to_value(*body).1,
                Value::make_list(catch_list),
            ];
            (loc, Value::make_list(list))
        }
        Ast::Yield(loc, val) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("co-yield")),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::CoResume(loc, co, arg) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("co-resume")),
                ast_to_value(*co).1,
                ast_to_value(*arg).1,
            ]),
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
        Ast::Match(loc, target, clauses) => {
            let mut list = vec![Value::Symbol(intern("match")), ast_to_value(*target).1];
            for c in clauses {
                let mut c_items = vec![pattern_to_value(c.pattern)];
                if let Some(guard) = c.guard {
                    c_items.push(Value::make_list(vec![
                        Value::Symbol(intern("where")),
                        ast_to_value(guard).1,
                    ]));
                }
                c_items.extend(c.body.into_iter().map(|b| ast_to_value(b).1));
                list.push(Value::make_list(c_items));
            }
            (loc, Value::make_list(list))
        }
        Ast::DotAccess(loc, expr, field_sym) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("rget")),
                ast_to_value(*expr).1,
                Value::Symbol(field_sym),
            ]),
        ),
        Ast::Pipeline(loc, left, right, _kind) => (
            loc,
            Value::make_list(vec![
                ast_to_value(*right).1,
                ast_to_value(*left).1,
            ]),
        ),
        Ast::TypeSignature(loc, id, sig) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("type-sig")),
                Value::Symbol(id),
                Value::make_string(&sig.fn_type.to_string()),
            ]),
        ),
        Ast::TypeAssert(loc, expr, ty) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("type-assert")),
                ast_to_value(*expr).1,
                Value::make_string(&ty.to_string()),
            ]),
        ),
        Ast::Newtype(loc, id, ty) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("newtype")),
                Value::Symbol(id),
                Value::make_string(&ty.to_string()),
            ]),
        ),
        Ast::Derive(loc, id, tr) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("derive")),
                Value::Symbol(id),
                Value::Symbol(tr),
            ]),
        ),
        Ast::Implements(loc, id, tr, handler) => (
            loc,
            Value::make_list(vec![
                Value::Symbol(intern("implements")),
                Value::Symbol(id),
                Value::Symbol(tr),
                ast_to_value(*handler).1,
            ]),
        ),
    }
}

pub fn pattern_to_value(pat: Pattern) -> Value {
    match pat {
        Pattern::Wildcard(_) => Value::Symbol(intern("_")),
        Pattern::Variable(_, id) => Value::Symbol(id),
        Pattern::Literal(_, ast) => ast_to_value(*ast).1,
        Pattern::List(_, pats) => {
            Value::make_list(pats.into_iter().map(pattern_to_value).collect())
        }
        Pattern::Cons(_, h, t) => Value::make_list(vec![
            Value::Symbol(intern("cons")),
            pattern_to_value(*h),
            pattern_to_value(*t),
        ]),
        Pattern::Rest(_, pfx, rest) => {
            let mut list: Vec<Value> = pfx.into_iter().map(pattern_to_value).collect();
            list.push(Value::Symbol(intern("&")));
            list.push(pattern_to_value(*rest));
            Value::make_list(list)
        }
        Pattern::Record(_, fields) => {
            let mut rec = Record::new();
            for (k, pat) in fields {
                rec.fields_mut().insert(k, pattern_to_value(pat));
            }
            Value::Record(Rc::new(rec))
        }
        Pattern::Or(_, pats) => {
            let mut list = vec![Value::Symbol(intern("or"))];
            list.extend(pats.into_iter().map(pattern_to_value));
            Value::make_list(list)
        }
        Pattern::As(_, id, sub_pat) => Value::make_list(vec![
            Value::Symbol(intern("@")),
            Value::Symbol(id),
            pattern_to_value(*sub_pat),
        ]),
    }
}

pub fn value_to_ast(val: Value, loc: Loc) -> Result<Ast> {
    match val {
        Value::Nil => Ok(Ast::Nil(loc)),
        Value::Integer(i) => Ok(Ast::Integer(loc, i)),
        Value::Float(f) => Ok(Ast::Float(loc, f)),
        Value::Boolean(b) => Ok(Ast::Boolean(loc, b)),
        Value::Symbol(id) => Ok(Ast::Symbol(loc, id)),
        Value::Char(c) => Ok(Ast::Char(loc, c)),
        Value::String(s) => Ok(Ast::String(loc, s.as_str().to_string())),
        Value::List(l) => {
            if !l.is_empty() && l.iter().all(|v| matches!(v, Value::Char(_))) {
                let s_str: String = l
                    .iter()
                    .map(|v| match v {
                        Value::Char(c) => *c,
                        _ => unreachable!(),
                    })
                    .collect();
                Ok(Ast::String(loc, s_str))
            } else {
                let mut ast_list = Vec::new();
                for v in l.iter() {
                    ast_list.push(value_to_ast(v.clone(), loc)?);
                }
                if ast_list.is_empty() {
                    return Ok(Ast::Nil(loc));
                }
                Ok(Ast::List(loc, ast_list))
            }
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
