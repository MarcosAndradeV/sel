use crate::compiler::*;
use crate::diagnostics::*;
use crate::lexer::*;
use crate::runtime::*;

type Result<T> = std::result::Result<T, SelError>;

pub fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<Ast> {
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
            Ok(Ast::Quote(t.loc, Box::new(expr)))
        }
        TokenKind::Ampersand => {
            if let Ast::Symbol(loc, id) = parse_expr(tokens, pos)? {
                return Ok(Ast::Bind(loc, id));
            }
            Err(SelError {
                loc: t.loc,
                kind: SelErrorKind::Generic("Expected identifier after &".into()),
            })
        }
        TokenKind::QuasiQuote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::Quasiquote(t.loc, Box::new(expr)))
        }
        TokenKind::Unquote => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::Unquote(t.loc, Box::new(expr)))
        }
        TokenKind::UnquoteSplicing => {
            let expr = parse_expr(tokens, pos)?;
            Ok(Ast::UnquoteSplicing(t.loc, Box::new(expr)))
        }
        TokenKind::String => Ok(Ast::String(t.loc, t.source.clone())),
        TokenKind::Boolean => Ok(Ast::Boolean(t.loc, t.source == "#t")),
        TokenKind::Number(base) => {
            let s = match base {
                NumberBase::X => t.source.trim_start_matches("0x").trim_start_matches("0X"),
                NumberBase::B => t.source.trim_start_matches("0b").trim_start_matches("0B"),
                NumberBase::O => t.source.trim_start_matches("0o").trim_start_matches("0O"),
                NumberBase::D => &t.source,
            };

            if let Ok(i) = i64::from_str_radix(s, base.radix()) {
                Ok(Ast::Integer(t.loc, i))
            } else if base == NumberBase::D {
                if let Ok(f) = t.source.parse::<f64>() {
                    Ok(Ast::Float(t.loc, f))
                } else {
                    Err(SelError {
                        loc: t.loc,
                        kind: SelErrorKind::InvalidNumber(t.source.clone()),
                    })
                }
            } else {
                Err(SelError {
                    loc: t.loc,
                    kind: SelErrorKind::InvalidNumber(t.source.clone()),
                })
            }
        }
        TokenKind::Identifier => match t.source.as_str() {
            "nil" => Ok(Ast::Nil(t.loc)),
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
            loc: open_token.loc,
            kind: SelErrorKind::Generic("Missing closing parenthesis".into()),
        });
    }
    *pos += 1; // consume ')'
    optimize_ast(list, open_token.loc)
}

fn optimize_ast(list: Vec<Ast>, loc: Loc) -> Result<Ast> {
    if list.is_empty() {
        return Ok(Ast::Nil(loc));
    }

    if let Some(Ast::Symbol(s_loc, id)) = list.first().cloned() {
        match lookup(id).as_str() {
            "if" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Missing condition in if".into()),
                })?;
                let true_branch = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
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
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Missing parameters in lambda".into()),
                })?;
                let mut params = Vec::new();
                match params_ast {
                    Ast::List(loc, p) => {
                        for param in p {
                            if let Ast::Symbol(_, id) = param {
                                params.push(id);
                            } else if let Ast::Bind(_, id) = param {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            } else {
                                return Err(SelError {
                                    loc,
                                    kind: SelErrorKind::Generic(
                                        "Expected identifier in lambda parameters".into(),
                                    ),
                                });
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError {
                            loc: s_loc,
                            kind: SelErrorKind::Generic(
                                "Expected parameter list in lambda".into(),
                            ),
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
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected identifier in define".into()),
                })?;

                if let Ast::List(loc, mut p_list) = name_ast {
                    if p_list.is_empty() {
                        return Err(SelError {
                            loc,
                            kind: SelErrorKind::Generic(
                                "Empty parameter list in define".into(),
                            ),
                        });
                    }
                    let head = p_list.remove(0);
                    let Ast::Symbol(_, name_id) = head else {
                        return Err(SelError {
                            loc: s_loc,
                            kind: SelErrorKind::Generic(
                                "Expected identifier at head of parameter list in define"
                                    .into(),
                            ),
                        });
                    };
                    let mut params = Vec::new();
                    for p in p_list {
                        match p {
                            Ast::Symbol(_, id) => params.push(id),
                            Ast::Bind(_, id) => {
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
                        s_loc,
                        name_id,
                        Box::new(Ast::Lambda(s_loc, params, body)),
                    ));
                }

                let value_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected expression in define".into()),
                })?;
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError {
                        loc: s_loc,
                        kind: SelErrorKind::Generic("Expected identifier in define".into()),
                    });
                };
                Ok(Ast::Define(s_loc, name_id, Box::new(value_ast)))
            }
            "defmacro" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected identifier in defmacro".into()),
                })?;
                let params_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected parameters in defmacro".into()),
                })?;

                let mut params = Vec::new();
                match params_ast {
                    Ast::List(_, p) => {
                        for param in p {
                            if let Ast::Symbol(_, id) = param {
                                params.push(id);
                            } else if let Ast::Bind(_, id) = param {
                                let name = lookup(id);
                                params.push(intern(&format!("&{}", name)));
                            } else {
                                return Err(SelError {
                                    loc: s_loc,
                                    kind: SelErrorKind::Generic(
                                        "Expected identifier in defmacro parameters".into(),
                                    ),
                                });
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError {
                            loc: s_loc,
                            kind: SelErrorKind::Generic(
                                "Expected parameter list in defmacro".into(),
                            ),
                        });
                    }
                }
                let body: Vec<Ast> = iter.collect();
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError {
                        loc: s_loc,
                        kind: SelErrorKind::Generic("Expected identifier in defmacro".into()),
                    });
                };
                Ok(Ast::DefMacro(
                    s_loc,
                    name_id,
                    Box::new(Ast::Lambda(s_loc, params, body)),
                ))
            }
            "set!" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected identifier in set!".into()),
                })?;
                let value_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected expression in set!".into()),
                })?;
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError {
                        loc: s_loc,
                        kind: SelErrorKind::Generic("Expected identifier in set!".into()),
                    });
                };
                Ok(Ast::Set(s_loc, name_id, Box::new(value_ast)))
            }
            "let" => {
                let mut iter = list.into_iter().skip(1);
                let bindings_ast = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected bindings in let".into()),
                })?;
                let mut bindings = Vec::new();
                match bindings_ast {
                    Ast::List(loc, b) => {
                        for bind in b {
                            if let Ast::List(loc, mut pair) = bind {
                                if pair.len() != 2 {
                                    return Err(SelError {
                                        loc,
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
                                        loc,
                                        kind: SelErrorKind::Generic(
                                            "Expected identifier in let binding".into(),
                                        ),
                                    });
                                }
                            } else {
                                return Err(SelError {
                                    loc,
                                    kind: SelErrorKind::Generic(
                                        "Expected binding pair in let".into(),
                                    ),
                                });
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError {
                            loc: s_loc,
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
                    loc: s_loc,
                    kind: SelErrorKind::Generic("Expected expression in quote".into()),
                })?;
                Ok(Ast::Quote(s_loc, Box::new(expr)))
            }
            "quasiquote" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().ok_or_else(|| SelError {
                    loc: s_loc,
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
            _ => Ok(Ast::List(loc, list)),
        }
    } else {
        Ok(Ast::List(loc, list))
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
    Bind(Loc, u32),
    Nil(Loc),
    Symbol(Loc, u32),
    Integer(Loc, i64),
    Float(Loc, f64),
    String(Loc, String),
    Boolean(Loc, bool),
    List(Loc, Vec<Self>),
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
            Ast::Nil(_) => write!(f, "nil"),
            Ast::Symbol(_, id) => write!(f, "{}", lookup(*id)),
            Ast::Integer(_, i) => write!(f, "{i}"),
            Ast::Float(_, n) => write!(f, "{n}"),
            Ast::String(_, s) => write!(f, "\"{s}\""),
            Ast::Boolean(_, b) => write!(f, "{}", if *b { "#t" } else { "#f" }),
            Ast::List(..) => write!(f, "<list>"),
            Ast::Bind(_, id) => write!(f, "&{}", lookup(*id)),
        }
    }
}

pub fn ast_to_value(ast: Ast) -> (Loc, Value) {
    match ast {
        Ast::Symbol(loc, id) => (loc, Value::Symbol(id)),
        Ast::Integer(loc, i) => (loc, Value::Integer(i)),
        Ast::Float(loc, f) => (loc, Value::Float(f)),
        Ast::String(loc, s) => (loc, Value::String(s)),
        Ast::Boolean(loc, b) => (loc, Value::Boolean(b)),
        Ast::Nil(loc) => (loc, Value::Nil),
        Ast::List(loc, l) => (
            loc,
            Value::List(l.into_iter().map(|a| ast_to_value(a).1).collect()),
        ),
        Ast::Define(loc, id, val) => (
            loc,
            Value::List(vec![
                Value::Symbol(intern("define")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::DefMacro(loc, id, val) => (
            loc,
            Value::List(vec![
                Value::Symbol(intern("defmacro")),
                Value::Symbol(id),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::Set(loc, id, val) => (
            loc,
            Value::List(vec![
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
            (loc, Value::List(list))
        }
        Ast::Lambda(loc, params, body) => {
            let mut list = vec![
                Value::Symbol(intern("lambda")),
                Value::List(params.into_iter().map(Value::Symbol).collect()),
            ];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list))
        }
        Ast::Begin(loc, body) => {
            let mut list = vec![Value::Symbol(intern("begin"))];
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list))
        }
        Ast::Let(loc, bindings, body) => {
            let mut list = vec![Value::Symbol(intern("let"))];
            let mut bind_list = Vec::new();
            for (id, val) in bindings {
                bind_list.push(Value::List(vec![Value::Symbol(id), ast_to_value(val).1]));
            }
            list.push(Value::List(bind_list));
            list.extend(body.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list))
        }
        Ast::Quote(loc, val) => (
            loc,
            Value::List(vec![Value::Symbol(intern("quote")), ast_to_value(*val).1]),
        ),
        Ast::Quasiquote(loc, val) => (
            loc,
            Value::List(vec![
                Value::Symbol(intern("quasiquote")),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::Unquote(loc, val) => (
            loc,
            Value::List(vec![Value::Symbol(intern("unquote")), ast_to_value(*val).1]),
        ),
        Ast::UnquoteSplicing(loc, val) => (
            loc,
            Value::List(vec![
                Value::Symbol(intern("unquote-splicing")),
                ast_to_value(*val).1,
            ]),
        ),
        Ast::And(loc, exprs) => {
            let mut list = vec![Value::Symbol(intern("and"))];
            list.extend(exprs.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list))
        }
        Ast::Or(loc, exprs) => {
            let mut list = vec![Value::Symbol(intern("or"))];
            list.extend(exprs.into_iter().map(|a| ast_to_value(a).1));
            (loc, Value::List(list))
        }
        Ast::Bind(loc, id) => (loc, Value::Symbol(intern(&format!("&{}", lookup(id))))),
    }
}

pub fn value_to_ast(val: Value, loc: Loc) -> Result<Ast> {
    match val {
        Value::Nil => Ok(Ast::Nil(loc)),
        Value::Integer(i) => Ok(Ast::Integer(loc, i)),
        Value::Float(f) => Ok(Ast::Float(loc, f)),
        Value::String(s) => Ok(Ast::String(loc, s)),
        Value::Boolean(b) => Ok(Ast::Boolean(loc, b)),
        Value::Symbol(id) => Ok(Ast::Symbol(loc, id)),
        Value::List(l) => {
            let mut ast_list = Vec::new();
            for v in l {
                ast_list.push(value_to_ast(v, loc)?);
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
        v => Err(SelError {
            loc,
            kind: SelErrorKind::Generic(format!(
                "Cannot convert function or macro to AST ({v})"
            )),
        }),
    }
}
