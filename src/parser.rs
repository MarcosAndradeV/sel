use crate::ast::Ast;
use crate::diagnostics::SelError;
use crate::lexer::{Lexer, Loc, NumberBase, Token, TokenKind};
use crate::types::{intern, lookup};

type Result<T> = std::result::Result<T, SelError>;

pub fn optimize_ast(list: Vec<Ast>, loc: Loc) -> Result<Ast> {
    if list.is_empty() {
        return Ok(Ast::Nil(loc));
    }

    if let Some(Ast::Symbol(s_loc, id)) = list.first().cloned() {
        match lookup(id).as_str() {
            "co-yield" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().unwrap_or(Ast::Nil(s_loc));
                Ok(Ast::Yield(s_loc, Box::new(expr)))
            }
            "co-resume" => {
                let mut iter = list.into_iter().skip(1);
                let co = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing coroutine in co-resume".into())
                })?;
                let arg = iter.next().unwrap_or(Ast::Nil(s_loc));
                Ok(Ast::CoResume(s_loc, Box::new(co), Box::new(arg)))
            }
            "try" => {
                let mut iter = list.into_iter().skip(1);
                let body = iter
                    .next()
                    .ok_or_else(|| SelError::SyntaxError(s_loc, "Missing body in try".into()))?;
                let catch_clause = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing catch clause in try".into())
                })?;

                match catch_clause {
                    Ast::List(c_loc, c_list) => {
                        let mut c_iter = c_list.into_iter();
                        let first = c_iter.next().ok_or_else(|| {
                            SelError::SyntaxError(
                                c_loc,
                                "Expected (catch err-var ...) clause".into(),
                            )
                        })?;

                        match first {
                            Ast::Symbol(_, catch_sym_id) if lookup(catch_sym_id) == "catch" => {
                                let err_var = c_iter.next().ok_or_else(|| {
                                    SelError::SyntaxError(
                                        c_loc,
                                        "Expected error variable in catch clause".into(),
                                    )
                                })?;
                                let err_var_id = match err_var {
                                    Ast::Symbol(_, id) => id,
                                    _ => {
                                        return Err(SelError::SyntaxError(
                                            c_loc,
                                            "Expected symbol for error variable".into(),
                                        ));
                                    }
                                };
                                let catch_body: Vec<Ast> = c_iter.collect();
                                Ok(Ast::Try(s_loc, Box::new(body), err_var_id, catch_body))
                            }
                            _ => Err(SelError::SyntaxError(
                                c_loc,
                                "Expected catch keyword as first element of catch clause".into(),
                            )),
                        }
                    }
                    _ => Err(SelError::SyntaxError(
                        s_loc,
                        "Expected catch clause to be a list".into(),
                    )),
                }
            }
            "->" => {
                let mut iter = list.into_iter().skip(1);
                let mut first_v = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing first expression in `->`".into())
                })?;
                for ast in iter {
                    match ast {
                        Ast::List(loc, mut list) => {
                            list.push(first_v);
                            first_v = optimize_ast(list, loc)?;
                        }
                        s => {
                            first_v = optimize_ast(vec![s, first_v], loc)?;
                        }
                    }
                }
                Ok(first_v)
            }
            "import" => {
                let mut iter = list.into_iter().skip(1);
                let first = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected module name in import".into())
                })?;

                match first {
                    Ast::Symbol(_, symbol) => {
                        // Check if there's an inline :as alias, e.g. (import foo :as f)
                        if let Some(next) = iter.next() {
                            if let Ast::Symbol(_, as_sym) = next {
                                if lookup(as_sym) == ":as" {
                                    let alias_ast = iter.next().ok_or_else(|| {
                                        SelError::SyntaxError(
                                            s_loc,
                                            "Expected alias after :as".into(),
                                        )
                                    })?;
                                    if let Ast::Symbol(_, alias) = alias_ast {
                                        Ok(Ast::Import(s_loc, symbol, Some(alias)))
                                    } else {
                                        Err(SelError::SyntaxError(
                                            s_loc,
                                            "Expected symbol for alias".into(),
                                        ))
                                    }
                                } else {
                                    Err(SelError::SyntaxError(
                                        s_loc,
                                        "Expected :as keyword for alias".into(),
                                    ))
                                }
                            } else {
                                Err(SelError::SyntaxError(
                                    s_loc,
                                    "Expected symbol for alias keyword".into(),
                                ))
                            }
                        } else {
                            Ok(Ast::Import(s_loc, symbol, None))
                        }
                    }
                    Ast::List(_, inner_list) => {
                        // e.g. (import (foo :as f)) or (import (foo f))
                        if inner_list.is_empty() {
                            return Err(SelError::SyntaxError(s_loc, "Empty import list".into()));
                        }
                        let mut inner_iter = inner_list.into_iter();
                        let first_inner = inner_iter.next().unwrap();
                        let Ast::Symbol(_, symbol) = first_inner else {
                            return Err(SelError::SyntaxError(
                                s_loc,
                                "Expected module name as symbol in import list".into(),
                            ));
                        };
                        if let Some(next) = inner_iter.next() {
                            match next {
                                Ast::Symbol(_, as_sym) if lookup(as_sym) == ":as" => {
                                    let alias_ast = inner_iter.next().ok_or_else(|| {
                                        SelError::SyntaxError(
                                            s_loc,
                                            "Expected alias after :as".into(),
                                        )
                                    })?;
                                    if let Ast::Symbol(_, alias) = alias_ast {
                                        Ok(Ast::Import(s_loc, symbol, Some(alias)))
                                    } else {
                                        Err(SelError::SyntaxError(
                                            s_loc,
                                            "Expected symbol for alias".into(),
                                        ))
                                    }
                                }
                                Ast::Symbol(_, alias) => {
                                    // e.g. (import (foo f))
                                    Ok(Ast::Import(s_loc, symbol, Some(alias)))
                                }
                                _ => Err(SelError::SyntaxError(
                                    s_loc,
                                    "Expected alias or :as keyword".into(),
                                )),
                            }
                        } else {
                            Ok(Ast::Import(s_loc, symbol, None))
                        }
                    }
                    _ => Err(SelError::SyntaxError(
                        s_loc,
                        "Expected symbol or list in import".into(),
                    )),
                }
            }
            "load" => {
                let mut iter = list.into_iter().skip(1);
                let path = iter
                    .next()
                    .ok_or_else(|| SelError::SyntaxError(s_loc, "Missing path in load".into()))?;
                if iter.next().is_some() {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected exactly 1 argument for load".into(),
                    ));
                }
                Ok(Ast::Load(s_loc, Box::new(path)))
            }
            "while" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in while".into())
                })?;
                let body: Vec<Ast> = iter.collect();
                Ok(Ast::While(
                    s_loc,
                    Box::new(cond),
                    Box::new(Ast::List(loc, body)),
                ))
            }
            "until" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in until".into())
                })?;
                let body: Vec<Ast> = iter.collect();
                Ok(Ast::Until(
                    s_loc,
                    Box::new(cond),
                    Box::new(Ast::List(loc, body)),
                ))
            }
            "if" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in if".into())
                })?;
                let true_branch = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing true branch in if".into())
                })?;
                let false_branch = iter.next();
                Ok(Ast::If(
                    s_loc,
                    Box::new(cond),
                    Box::new(true_branch),
                    false_branch.map(Box::new),
                ))
            }
            "cond" => {
                let mut iter = list.into_iter().skip(1);
                let mut branches = Vec::new();
                loop {
                    let Some(cond) = iter.next() else {
                        break;
                    };
                    let expr = iter.next().ok_or_else(|| {
                        SelError::SyntaxError(s_loc, "Missing expr in cond".into())
                    })?;
                    branches.push((cond, expr));
                }
                Ok(Ast::Cond(s_loc, branches))
            }
            "ffi-func" => {
                // (define puts (ffi-func 'i32 '('*u8)))
                let mut iter = list.into_iter().skip(1);
                let sym = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing ffi-sym in ffi-func".into())
                })?;
                let ret = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing return type in ffi-func".into())
                })?;
                let arg_types = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing argument types in ffi-func".into())
                })?;
                // ffi-call ~sym ~ret ~arg-types args
                Ok(Ast::Lambda(
                    s_loc,
                    vec![intern("&args")],
                    vec![Ast::List(
                        s_loc,
                        vec![
                            Ast::Symbol(s_loc, intern("ffi-call")),
                            sym,
                            ret,
                            arg_types,
                            Ast::Symbol(s_loc, intern("args")),
                        ],
                    )],
                ))
            }
            "unless" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in unless".into())
                })?;
                let false_branch = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing false branch in unless".into())
                })?;
                let true_branch = iter.next();
                Ok(Ast::Unless(
                    s_loc,
                    Box::new(cond),
                    Box::new(false_branch),
                    true_branch.map(Box::new),
                ))
            }
            "when" => {
                let mut iter = list.into_iter().skip(1);
                let cond = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing condition in when".into())
                })?;
                let body = iter.collect();
                Ok(Ast::When(s_loc, Box::new(cond), body))
            }
            "lambda" => {
                let mut iter = list.into_iter().skip(1);
                let params_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Missing parameters in lambda".into())
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
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected identifier in lambda parameters".into(),
                                ));
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Expected parameter list in lambda".into(),
                        ));
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
                let name_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected identifier in define".into())
                })?;
                if let Ast::List(l_loc, mut p_list) = name_ast {
                    if p_list.is_empty() {
                        return Err(SelError::SyntaxError(
                            l_loc,
                            "Empty parameter list in define".into(),
                        ));
                    }
                    let head = p_list.remove(0);
                    let Ast::Symbol(_, name_id) = head else {
                        return Err(SelError::SyntaxError(
                            l_loc,
                            "Expected identifier at head of parameter list in define".into(),
                        ));
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
                                return Err(SelError::SyntaxError(
                                    l_loc,
                                    "Expected identifier in parameter list".into(),
                                ));
                            }
                        }
                    }
                    let body: Vec<Ast> = iter.collect();
                    if body.is_empty() {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Missing body in define".into(),
                        ));
                    }
                    return Ok(Ast::Define(
                        s_loc,
                        name_id,
                        Box::new(Ast::Lambda(s_loc, params, body)),
                    ));
                }

                let value_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in define".into())
                })?;
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected identifier in define".into(),
                    ));
                };
                Ok(Ast::Define(s_loc, name_id, Box::new(value_ast)))
            }
            "defmacro" => {
                let mut iter = list.into_iter().skip(1);
                let name_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected identifier in defmacro".into())
                })?;
                let params_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected parameters in defmacro".into())
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
                                return Err(SelError::SyntaxError(
                                    param.loc(),
                                    "Expected identifier in defmacro parameters".into(),
                                ));
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Expected parameter list in defmacro".into(),
                        ));
                    }
                }
                let body: Vec<Ast> = iter.collect();
                let Ast::Symbol(_, name_id) = name_ast else {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected identifier in defmacro".into(),
                    ));
                };
                Ok(Ast::DefMacro(
                    s_loc,
                    name_id,
                    Box::new(Ast::Lambda(s_loc, params, body)),
                ))
            }
            "set!" => {
                let mut iter = list.into_iter().skip(1);
                let Some(Ast::Symbol(_, name_id)) = iter.next() else {
                    return Err(SelError::SyntaxError(
                        s_loc,
                        "Expected identifier in set!".into(),
                    ));
                };
                let value_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in set!".into())
                })?;
                Ok(Ast::Set(s_loc, name_id, Box::new(value_ast)))
            }
            "let" => {
                let mut iter = list.into_iter().skip(1);
                let bindings_ast = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected bindings in let".into())
                })?;
                let mut bindings = Vec::new();
                match bindings_ast {
                    Ast::List(loc, b) => {
                        for bind in b {
                            if let Ast::List(loc, mut pair) = bind {
                                if pair.len() != 2 {
                                    return Err(SelError::SyntaxError(
                                        loc,
                                        "Invalid binding pair in let".into(),
                                    ));
                                }
                                let val = pair.pop().unwrap();
                                let name = pair.pop().unwrap();
                                if let Ast::Symbol(_, name_id) = name {
                                    bindings.push((name_id, val));
                                } else {
                                    return Err(SelError::SyntaxError(
                                        loc,
                                        "Expected identifier in let binding".into(),
                                    ));
                                }
                            } else {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected binding pair in let".into(),
                                ));
                            }
                        }
                    }
                    Ast::Nil(_) => {}
                    _ => {
                        return Err(SelError::SyntaxError(
                            s_loc,
                            "Expected binding list in let".into(),
                        ));
                    }
                }
                let body = iter.collect();
                Ok(Ast::Let(s_loc, bindings, body))
            }
            "quote" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in quote".into())
                })?;
                Ok(Ast::Quote(s_loc, Box::new(expr)))
            }
            "quasiquote" => {
                let mut iter = list.into_iter().skip(1);
                let expr = iter.next().ok_or_else(|| {
                    SelError::SyntaxError(s_loc, "Expected expression in quasiquote".into())
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

pub fn parse_all(line: &str, file_id: u32, diags: &mut Vec<SelError>) -> Vec<Ast> {
    let mut lex = Lexer::new(line, file_id);
    let mut tokens = Vec::new();
    loop {
        match lex.next_token() {
            Ok(Some(t)) => tokens.push(t),
            Ok(None) => break,
            Err(e) => {
                diags.push(e);
            }
        }
    }
    let mut pos = 0;
    let mut asts = Vec::new();
    while pos < tokens.len() {
        match parse_expr(&tokens, &mut pos, diags) {
            Ok(ast) => asts.push(ast),
            Err(e) => {
                diags.push(e);
                recover_parser_state(&tokens, &mut pos);
            }
        }
    }
    asts
}

pub fn parse_expr(tokens: &[Token], pos: &mut usize, diags: &mut Vec<SelError>) -> Result<Ast> {
    if *pos >= tokens.len() {
        return Err(SelError::UnexpectedEOF(
            tokens.last().map(|t| t.loc).unwrap_or_default(),
        ));
    }
    let t = &tokens[*pos];
    *pos += 1;

    match t.kind {
        TokenKind::Bind => Err(SelError::SyntaxError(t.loc, "Unexpected `:=`".to_string())),
        TokenKind::BackSlash => parse_lambda_shorthand(tokens, pos, t, diags),
        TokenKind::OpenCurly => parse_record(tokens, pos, t, diags),
        TokenKind::OpenParen => parse_list(tokens, pos, t, diags),
        TokenKind::CloseParen => Err(SelError::SyntaxError(t.loc, "Unexpected `)`".to_string())),
        TokenKind::CloseCurly => Err(SelError::SyntaxError(t.loc, "Unexpected `}`".to_string())),
        TokenKind::Quote => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::Quote(t.loc, Box::new(expr)))
        }
        TokenKind::Ampersand => {
            let expr = parse_expr(tokens, pos, diags)?;
            if let Ast::Symbol(loc, id) = expr {
                return Ok(Ast::Bind(loc, id));
            }
            Err(SelError::SyntaxError(
                t.loc,
                "Expected identifier after &".into(),
            ))
        }
        TokenKind::QuasiQuote => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::Quasiquote(t.loc, Box::new(expr)))
        }
        TokenKind::Unquote => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::Unquote(t.loc, Box::new(expr)))
        }
        TokenKind::UnquoteSplicing => {
            let expr = parse_expr(tokens, pos, diags)?;
            Ok(Ast::UnquoteSplicing(t.loc, Box::new(expr)))
        }
        TokenKind::Identifier => match t.source.as_str() {
            "nil" => Ok(Ast::Nil(t.loc)),
            ":private" => Ok(Ast::VisibilityDirective(t.loc, false)),
            ":public" => Ok(Ast::VisibilityDirective(t.loc, true)),
            ":do" => {
                *pos += 1;
                let expr = parse_list_expr(tokens, pos, t, diags)?;
                Ok(Ast::Begin(t.loc, expr))
            }
            _ => {
                if let Some(tb) = tokens.get(*pos)
                    && tb.kind == TokenKind::Bind
                {
                    *pos += 1;
                    let expr = parse_expr(tokens, pos, diags)?;
                    Ok(Ast::Define(t.loc, intern(&t.source), Box::new(expr)))
                } else {
                    Ok(Ast::Symbol(t.loc, intern(&t.source)))
                }
            }
        },
        TokenKind::Number(base) => {
            let s = match base {
                NumberBase::X => t.source.trim_start_matches("0x").trim_start_matches("0X"),
                NumberBase::B => t.source.trim_start_matches("0b").trim_start_matches("0B"),
                NumberBase::O => t.source.trim_start_matches("0o").trim_start_matches("0O"),
                NumberBase::D => &t.source,
            };

            if let Ok(i) = i64::from_str_radix(s, base.radix()) {
                return Ok(Ast::Integer(t.loc, i));
            } else if base == NumberBase::D
                && let Ok(f) = t.source.parse::<f64>()
            {
                return Ok(Ast::Float(t.loc, f));
            }
            Err(SelError::InvalidNumber(t.clone()))
        }
        TokenKind::String => Ok(Ast::String(t.loc, t.source.clone())),
        TokenKind::Boolean => Ok(Ast::Boolean(t.loc, t.source == "#t" || t.source == "#true")),
        TokenKind::Char(c) => Ok(Ast::Char(t.loc, c)),
    }
}

fn parse_lambda_shorthand(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Ast> {
    let args = parse_expr(tokens, pos, diags)?;
    let body = parse_expr(tokens, pos, diags)?;
    Ok(Ast::List(
        open_token.loc,
        vec![Ast::Symbol(open_token.loc, intern("lambda")), args, body],
    ))
}

fn parse_record(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Ast> {
    let mut record = Vec::new();
    while *pos < tokens.len() && tokens[*pos].kind != TokenKind::CloseCurly {
        let sym_expr = match parse_expr(tokens, pos, diags) {
            Ok(ast) => ast,
            Err(e) => {
                diags.push(e);
                recover_parser_state(tokens, pos);
                continue;
            }
        };
        let sym = match sym_expr {
            Ast::Symbol(_, sym) => sym,
            ast => {
                diags.push(SelError::SyntaxError(
                    ast.loc(),
                    format!("Expected identifier-value pair in records found {ast}"),
                ));
                recover_parser_state(tokens, pos);
                continue;
            }
        };
        let v_expr = match parse_expr(tokens, pos, diags) {
            Ok(ast) => ast,
            Err(e) => {
                diags.push(e);
                recover_parser_state(tokens, pos);
                continue;
            }
        };
        record.push((sym, v_expr));
    }
    if *pos >= tokens.len() {
        return Err(SelError::SyntaxError(
            open_token.loc,
            "Missing closing curly brace".into(),
        ));
    }
    *pos += 1; // consume '}'
    Ok(Ast::Record(open_token.loc, record))
}

fn parse_list(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Ast> {
    let list = parse_list_expr(tokens, pos, open_token, diags)?;
    Ok(Ast::List(open_token.loc, list))
}

fn parse_list_expr(
    tokens: &[Token],
    pos: &mut usize,
    open_token: &Token,
    diags: &mut Vec<SelError>,
) -> Result<Vec<Ast>> {
    let mut list = Vec::new();
    while *pos < tokens.len() && tokens[*pos].kind != TokenKind::CloseParen {
        match parse_expr(tokens, pos, diags) {
            Ok(ast) => list.push(ast),
            Err(e) => {
                diags.push(e);
                recover_parser_state(tokens, pos);
            }
        }
    }
    if *pos >= tokens.len() {
        return Err(SelError::SyntaxError(
            open_token.loc,
            "Missing closing parenthesis".into(),
        ));
    }
    *pos += 1;
    Ok(list)
}

fn recover_parser_state(tokens: &[Token], pos: &mut usize) {
    let mut depth = 0;
    while *pos < tokens.len() {
        let t = &tokens[*pos];
        match t.kind {
            TokenKind::OpenParen | TokenKind::OpenCurly => {
                depth += 1;
                *pos += 1;
            }
            TokenKind::CloseParen | TokenKind::CloseCurly => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                *pos += 1;
            }
            _ => {
                if depth == 0 {
                    break;
                }
                *pos += 1;
            }
        }
    }
}

pub fn resolve_ast(ast: Ast) -> Result<Ast> {
    match ast {
        Ast::List(loc, list) => {
            if list.is_empty() {
                return Ok(Ast::Nil(loc));
            }
            let mut resolved_list = Vec::with_capacity(list.len());
            for item in list {
                resolved_list.push(resolve_ast(item)?);
            }
            optimize_ast(resolved_list, loc)
        }
        Ast::Quasiquote(loc, expr) => {
            Ok(Ast::Quasiquote(loc, Box::new(resolve_quasiquote(*expr)?)))
        }
        Ast::Unquote(loc, expr) => Ok(Ast::Unquote(loc, Box::new(resolve_ast(*expr)?))),
        Ast::UnquoteSplicing(loc, expr) => {
            Ok(Ast::UnquoteSplicing(loc, Box::new(resolve_ast(*expr)?)))
        }
        Ast::Record(loc, fields) => {
            let mut resolved_fields = Vec::with_capacity(fields.len());
            for (k, v) in fields {
                resolved_fields.push((k, resolve_ast(v)?));
            }
            Ok(Ast::Record(loc, resolved_fields))
        }
        Ast::Define(loc, id, expr) => Ok(Ast::Define(loc, id, Box::new(resolve_ast(*expr)?))),
        Ast::Set(loc, id, expr) => Ok(Ast::Set(loc, id, Box::new(resolve_ast(*expr)?))),
        Ast::Let(loc, bindings, body) => {
            let mut resolved_bindings = Vec::with_capacity(bindings.len());
            for (id, val) in bindings {
                resolved_bindings.push((id, resolve_ast(val)?));
            }
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::Let(loc, resolved_bindings, resolved_body))
        }
        Ast::When(loc, cond, body) => {
            let resolved_cond = resolve_ast(*cond)?;
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::When(loc, Box::new(resolved_cond), resolved_body))
        }
        Ast::Unless(loc, cond, false_branch, true_branch) => {
            let resolved_cond = resolve_ast(*cond)?;
            let resolved_false = resolve_ast(*false_branch)?;
            let resolved_true = match true_branch {
                Some(b) => Some(Box::new(resolve_ast(*b)?)),
                None => None,
            };
            Ok(Ast::Unless(
                loc,
                Box::new(resolved_cond),
                Box::new(resolved_false),
                resolved_true,
            ))
        }
        Ast::If(loc, cond, true_branch, false_branch) => {
            let resolved_cond = resolve_ast(*cond)?;
            let resolved_true = resolve_ast(*true_branch)?;
            let resolved_false = match false_branch {
                Some(b) => Some(Box::new(resolve_ast(*b)?)),
                None => None,
            };
            Ok(Ast::If(
                loc,
                Box::new(resolved_cond),
                Box::new(resolved_true),
                resolved_false,
            ))
        }
        Ast::Try(loc, body, err_var, catch_body) => {
            let resolved_body = resolve_ast(*body)?;
            let mut resolved_catch = Vec::with_capacity(catch_body.len());
            for expr in catch_body {
                resolved_catch.push(resolve_ast(expr)?);
            }
            Ok(Ast::Try(
                loc,
                Box::new(resolved_body),
                err_var,
                resolved_catch,
            ))
        }
        Ast::Lambda(loc, params, body) => {
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::Lambda(loc, params, resolved_body))
        }
        Ast::DefMacro(loc, id, expr) => Ok(Ast::DefMacro(loc, id, Box::new(resolve_ast(*expr)?))),
        Ast::Begin(loc, body) => {
            let mut resolved_body = Vec::with_capacity(body.len());
            for expr in body {
                resolved_body.push(resolve_ast(expr)?);
            }
            Ok(Ast::Begin(loc, resolved_body))
        }
        Ast::Cond(loc, branches) => {
            let mut resolved_branches = Vec::with_capacity(branches.len());
            for (c, e) in branches {
                resolved_branches.push((resolve_ast(c)?, resolve_ast(e)?));
            }
            Ok(Ast::Cond(loc, resolved_branches))
        }
        Ast::Yield(loc, expr) => Ok(Ast::Yield(loc, Box::new(resolve_ast(*expr)?))),
        Ast::CoResume(loc, co, arg) => Ok(Ast::CoResume(
            loc,
            Box::new(resolve_ast(*co)?),
            Box::new(resolve_ast(*arg)?),
        )),
        Ast::Load(loc, path) => Ok(Ast::Load(loc, Box::new(resolve_ast(*path)?))),
        other => Ok(other),
    }
}

pub fn resolve_quasiquote(ast: Ast) -> Result<Ast> {
    match ast {
        Ast::Unquote(loc, expr) => Ok(Ast::Unquote(loc, Box::new(resolve_ast(*expr)?))),
        Ast::UnquoteSplicing(loc, expr) => {
            Ok(Ast::UnquoteSplicing(loc, Box::new(resolve_ast(*expr)?)))
        }
        Ast::List(loc, list) => {
            let mut resolved = Vec::with_capacity(list.len());
            for item in list {
                resolved.push(resolve_quasiquote(item)?);
            }
            Ok(Ast::List(loc, resolved))
        }
        Ast::Record(loc, fields) => {
            let mut resolved = Vec::with_capacity(fields.len());
            for (k, v) in fields {
                resolved.push((k, resolve_quasiquote(v)?));
            }
            Ok(Ast::Record(loc, resolved))
        }
        Ast::Load(loc, path) => Ok(Ast::Load(loc, Box::new(resolve_quasiquote(*path)?))),
        other => Ok(other),
    }
}
