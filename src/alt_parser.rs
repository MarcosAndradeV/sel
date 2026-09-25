//! Modern functional syntax parser for SEL using `lex-just-parse`.
//!
//! Features:
//! - Definitions: `mult x y := x * y`, `pub add x y := x + y`, `x := 10`
//! - Lambdas: `\x -> x * 2`, `\x y -> x + y`
//! - Scoping: `let x = 10, y = 20 in x + y`, `do ... end`
//! - Conditionals: `if cond then expr1 else expr2`
//! - Pattern matching: `match expr with | pat [when guard] -> body`
//! - Error handling: `try expr catch err -> handler`
//! - Coroutines: `yield expr`, `co_resume(co, val)`
//! - Pipeline: `x |> f(y)` (thread-last desugaring)
//! - Infix operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`
//! - Dot access: `record.field` -> `(rget record 'field)`
//! - Atoms: `:ok`, `:error`

use std::collections::HashSet;

use crate::ast::{Ast, MatchClause, Pattern};
use crate::diagnostics::SelError;
use crate::lexer::Loc;
use crate::types::intern;
use lex_just_parse::lexer::{Lexer, NumberBase, Token, TokenKind};

type Result<T> = std::result::Result<T, SelError>;

const KEYWORDS: &[&str] = &[
    "let", "in", "do", "end", "if", "then", "else", "match", "with", "when",
    "try", "catch", "yield", "pub", "import", "as", "true", "false", "nil",
];

fn to_loc(loc: lex_just_parse::lexer::Loc, file_id: u32) -> Loc {
    Loc {
        file_id,
        line: loc.line as u32,
        col: loc.col as u32,
    }
}

fn scan_escaped_identifiers(source: &str) -> (String, Vec<String>) {
    let mut result = String::with_capacity(source.len());
    let mut escaped = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while i < len {
        let c = chars[i];

        if in_str {
            result.push(c);
            if c == '\\' && i + 1 < len {
                i += 1;
                result.push(chars[i]);
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        if in_line_comment {
            result.push(c);
            if c == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            result.push(c);
            if c == '*' && i + 1 < len && chars[i + 1] == '/' {
                result.push('/');
                i += 2;
                in_block_comment = false;
                continue;
            }
            i += 1;
            continue;
        }

        // Check comment starts
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            result.push('/');
            result.push('/');
            i += 2;
            in_line_comment = true;
            continue;
        }
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            result.push('/');
            result.push('*');
            i += 2;
            in_block_comment = true;
            continue;
        }

        if c == '"' {
            result.push('"');
            in_str = true;
            i += 1;
            continue;
        }

        // Check for `:'...`
        if c == ':' && i + 1 < len && chars[i + 1] == '\'' {
            // Find closing single quote
            let mut j = i + 2;
            let mut name = String::new();
            let mut closed = false;
            while j < len {
                if chars[j] == '\\' && j + 1 < len {
                    name.push(chars[j + 1]);
                    j += 2;
                } else if chars[j] == '\'' {
                    closed = true;
                    j += 1;
                    break;
                } else {
                    name.push(chars[j]);
                    j += 1;
                }
            }

            if closed {
                let total_chars = j - i;
                let id = escaped.len();
                escaped.push(name);
                let tag = format!("__sel_esc_{id}_");
                let mut placeholder = tag;
                while placeholder.chars().count() < total_chars {
                    placeholder.push('_');
                }
                result.push_str(&placeholder);
                i = j;
                continue;
            }
        }

        result.push(c);
        i += 1;
    }

    (result, escaped)
}

pub struct AltParser<'a> {
    tokens: Vec<Token>,
    escaped_tokens: HashSet<usize>,
    pos: usize,
    file_id: u32,
    _source: &'a str,
}

impl<'a> AltParser<'a> {
    pub fn new(source: &'a str, file_id: u32) -> Self {
        let (sanitized, escaped_list) = scan_escaped_identifiers(source);
        let mut lexer = Lexer::new(&sanitized).with_keywords(KEYWORDS);
        let mut tokens = Vec::new();
        let mut escaped_tokens = HashSet::new();

        loop {
            let mut tok = lexer.next();
            if tok.is_eof() {
                tokens.push(tok);
                break;
            }

            if tok.kind == TokenKind::Identifier && tok.source().starts_with("__sel_esc_") {
                let s = tok.source();
                if let Some(rest) = s.strip_prefix("__sel_esc_") {
                    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(id) = num_str.parse::<usize>()
                        && id < escaped_list.len()
                    {
                        let name = &escaped_list[id];
                        tok = Token::new(TokenKind::Identifier, tok.loc, name.as_str().into());
                        escaped_tokens.insert(tokens.len());
                    }
                }
            }

            tokens.push(tok);
        }

        Self {
            tokens,
            escaped_tokens,
            pos: 0,
            file_id,
            _source: source,
        }
    }

    fn peek(&self) -> &Token {
        if self.pos >= self.tokens.len() {
            self.tokens.last().unwrap()
        } else {
            &self.tokens[self.pos]
        }
    }

    fn peek_ahead(&self, offset: usize) -> &Token {
        let idx = self.pos + offset;
        if idx >= self.tokens.len() {
            self.tokens.last().unwrap()
        } else {
            &self.tokens[idx]
        }
    }

    fn advance(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            tok
        } else {
            self.tokens.last().cloned().unwrap()
        }
    }

    fn current_loc(&self) -> Loc {
        to_loc(self.peek().loc, self.file_id)
    }

    fn at_eof(&self) -> bool {
        self.peek().is_eof()
    }

    fn match_keyword(&self, kw: &str) -> bool {
        let tok = self.peek();
        (tok.kind == TokenKind::Keyword || tok.kind == TokenKind::Identifier) && tok.source() == kw
    }

    fn eat_keyword(&mut self, kw: &str) -> bool {
        if self.match_keyword(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<Loc> {
        let loc = self.current_loc();
        if self.eat_keyword(kw) {
            Ok(loc)
        } else {
            Err(SelError::SyntaxError(
                loc,
                format!("Expected keyword `{kw}`, found `{}`", self.peek().source()),
            ))
        }
    }

    fn eat_token(&mut self, kind: TokenKind) -> bool {
        if self.peek().kind == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_token(&mut self, kind: TokenKind, expected_desc: &str) -> Result<Token> {
        let tok = self.peek();
        if tok.kind == kind {
            Ok(self.advance())
        } else {
            Err(SelError::SyntaxError(
                self.current_loc(),
                format!("Expected {expected_desc}, found `{}`", tok.source()),
            ))
        }
    }

    fn skip_semicolons(&mut self) {
        while self.peek().kind == TokenKind::SemiColon {
            self.advance();
        }
    }

    pub fn parse_program(&mut self, diags: &mut Vec<SelError>) -> Vec<Ast> {
        let mut asts = Vec::new();
        self.skip_semicolons();
        while !self.at_eof() {
            match self.parse_item() {
                Ok(ast) => asts.push(ast),
                Err(err) => {
                    diags.push(err);
                    self.recover();
                }
            }
            self.skip_semicolons();
        }
        asts
    }

    fn recover(&mut self) {
        while !self.at_eof() {
            let tok = self.peek();
            if tok.kind == TokenKind::SemiColon {
                self.advance();
                break;
            }
            if self.match_keyword("pub")
                || self.match_keyword("import")
                || self.match_keyword("let")
                || self.match_keyword("do")
                || self.match_keyword("end")
            {
                break;
            }
            self.advance();
        }
    }

    fn parse_item(&mut self) -> Result<Ast> {
        let is_pub = self.eat_keyword("pub");
        let start_loc = self.current_loc();

        if self.match_keyword("import") {
            let import_ast = self.parse_import(start_loc)?;
            return if is_pub {
                Ok(Ast::Begin(
                    start_loc,
                    vec![
                        Ast::VisibilityDirective(start_loc, true),
                        import_ast,
                        Ast::VisibilityDirective(start_loc, false),
                    ],
                ))
            } else {
                Ok(import_ast)
            };
        }

        // Check for definition: `name [params*] := expr`
        if let Some((name_sym, params)) = self.try_parse_def_header()? {
            self.expect_token(TokenKind::Assign, "`:=` in definition")?;
            let body = self.parse_expr()?;
            let def_ast = if params.is_empty() {
                Ast::Define(start_loc, name_sym, Box::new(body))
            } else {
                let lambda = Ast::Lambda(start_loc, params, vec![body]);
                Ast::Define(start_loc, name_sym, Box::new(lambda))
            };

            if is_pub {
                Ok(Ast::Begin(
                    start_loc,
                    vec![
                        Ast::VisibilityDirective(start_loc, true),
                        def_ast,
                        Ast::VisibilityDirective(start_loc, false),
                    ],
                ))
            } else {
                Ok(def_ast)
            }
        } else {
            if is_pub {
                return Err(SelError::SyntaxError(
                    start_loc,
                    "`pub` can only precede a definition or import".into(),
                ));
            }
            self.parse_expr()
        }
    }

    fn try_parse_def_header(&mut self) -> Result<Option<(u32, Vec<u32>)>> {
        // Look ahead to see if this is an identifier sequence followed by `:=`
        let mut offset = 0;
        let first = self.peek_ahead(offset);
        if first.kind != TokenKind::Identifier || KEYWORDS.contains(&first.source()) {
            return Ok(None);
        }

        offset += 1;
        let mut param_tokens = Vec::new();
        loop {
            let tok = self.peek_ahead(offset);
            if tok.kind == TokenKind::Identifier && !KEYWORDS.contains(&tok.source()) {
                param_tokens.push(tok.source().to_string());
                offset += 1;
            } else if tok.kind == TokenKind::Assign {
                // Verified definition!
                let name_token = self.advance();
                let name_sym = intern(name_token.source());
                let mut params = Vec::new();
                for _ in 0..param_tokens.len() {
                    let p_tok = self.advance();
                    params.push(intern(p_tok.source()));
                }
                return Ok(Some((name_sym, params)));
            } else {
                return Ok(None);
            }
        }
    }

    fn parse_import(&mut self, loc: Loc) -> Result<Ast> {
        self.expect_keyword("import")?;
        let name_tok = self.peek().clone();
        let mod_id = if name_tok.kind == TokenKind::StringLiteral {
            self.advance();
            intern(&name_tok.unescape())
        } else if name_tok.kind == TokenKind::Identifier {
            self.advance();
            intern(name_tok.source())
        } else {
            return Err(SelError::SyntaxError(
                loc,
                format!("Expected module name in import, found `{}`", name_tok.source()),
            ));
        };

        let alias = if self.eat_keyword("as") {
            let alias_tok = self.expect_token(TokenKind::Identifier, "alias identifier")?;
            Some(intern(alias_tok.source()))
        } else {
            None
        };

        Ok(Ast::Import(loc, mod_id, alias))
    }

    pub fn parse_expr(&mut self) -> Result<Ast> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Ast> {
        let mut left = self.parse_logical_and()?;
        while self.peek().kind == TokenKind::DoublePipe {
            let loc = self.current_loc();
            self.advance();
            let right = self.parse_logical_and()?;
            left = Ast::Or(loc, vec![left, right]);
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Ast> {
        let mut left = self.parse_equality()?;
        while self.peek().kind == TokenKind::DoubleAmpersand {
            let loc = self.current_loc();
            self.advance();
            let right = self.parse_equality()?;
            left = Ast::And(loc, vec![left, right]);
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Ast> {
        let mut left = self.parse_relational()?;
        loop {
            let kind = self.peek().kind;
            if kind == TokenKind::EqEq {
                let loc = self.current_loc();
                self.advance();
                let right = self.parse_relational()?;
                left = Ast::List(loc, vec![Ast::Symbol(loc, intern("eq?")), left, right]);
            } else if kind == TokenKind::NotEq {
                let loc = self.current_loc();
                self.advance();
                let right = self.parse_relational()?;
                let eq_ast = Ast::List(loc, vec![Ast::Symbol(loc, intern("eq?")), left, right]);
                left = Ast::List(loc, vec![Ast::Symbol(loc, intern("not")), eq_ast]);
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Ast> {
        let mut left = self.parse_pipeline()?;
        loop {
            let kind = self.peek().kind;
            let op_name = match kind {
                TokenKind::Lt => "<",
                TokenKind::LtEq => "<=",
                TokenKind::Gt => ">",
                TokenKind::GtEq => ">=",
                _ => break,
            };
            let loc = self.current_loc();
            self.advance();
            let right = self.parse_pipeline()?;
            left = Ast::List(loc, vec![Ast::Symbol(loc, intern(op_name)), left, right]);
        }
        Ok(left)
    }

    fn parse_pipeline(&mut self) -> Result<Ast> {
        let mut left = self.parse_additive()?;

        while self.peek().kind == TokenKind::Pipe && self.peek_ahead(1).kind == TokenKind::Gt {
            let pipe_loc = self.current_loc();
            self.advance(); // Pipe '|'
            self.advance(); // Gt '>'

            let right = self.parse_additive()?;
            left = self.desugar_pipe(pipe_loc, left, right);
        }

        Ok(left)
    }

    fn desugar_pipe(&self, loc: Loc, val: Ast, target: Ast) -> Ast {
        match target {
            Ast::List(l_loc, mut items) => {
                // Thread as last argument: f(a, b) with val -> f(a, b, val)
                items.push(val);
                Ast::List(l_loc, items)
            }
            other => {
                // Thread into function: target(val)
                Ast::List(loc, vec![other, val])
            }
        }
    }

    fn parse_additive(&mut self) -> Result<Ast> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let kind = self.peek().kind;
            let op_name = match kind {
                TokenKind::Plus => "+",
                TokenKind::Minus => "-",
                _ => break,
            };
            let loc = self.current_loc();
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Ast::List(loc, vec![Ast::Symbol(loc, intern(op_name)), left, right]);
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Ast> {
        let mut left = self.parse_unary()?;
        loop {
            let kind = self.peek().kind;
            let op_name = match kind {
                TokenKind::Asterisk => "*",
                TokenKind::Slash => "/",
                TokenKind::Mod => "mod",
                _ => break,
            };
            let loc = self.current_loc();
            self.advance();
            let right = self.parse_unary()?;
            left = Ast::List(loc, vec![Ast::Symbol(loc, intern(op_name)), left, right]);
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Ast> {
        let tok = self.peek();
        let loc = self.current_loc();
        if tok.kind == TokenKind::Minus {
            self.advance();
            let operand = self.parse_postfix()?;
            Ok(Ast::List(
                loc,
                vec![Ast::Symbol(loc, intern("-")), Ast::Integer(loc, 0), operand],
            ))
        } else if tok.kind == TokenKind::Bang {
            self.advance();
            let operand = self.parse_postfix()?;
            Ok(Ast::List(
                loc,
                vec![Ast::Symbol(loc, intern("not")), operand],
            ))
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<Ast> {
        let mut expr = self.parse_primary()?;

        loop {
            let tok = self.peek();
            if tok.kind == TokenKind::OpenParen {
                // Call: expr(arg1, arg2, ...)
                let loc = self.current_loc();
                self.advance();
                let mut args = vec![expr];
                if self.peek().kind != TokenKind::CloseParen {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.peek().kind == TokenKind::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect_token(TokenKind::CloseParen, "`)` after call arguments")?;
                expr = Ast::List(loc, args);
            } else if tok.kind == TokenKind::Dot {
                // Dot access: expr.field -> (rget expr 'field)
                let loc = self.current_loc();
                self.advance();
                let field_tok = self.expect_token(TokenKind::Identifier, "field name after `.`")?;
                let field_sym = intern(field_tok.source());
                expr = Ast::List(
                    loc,
                    vec![
                        Ast::Symbol(loc, intern("rget")),
                        expr,
                        Ast::Quote(loc, Box::new(Ast::Symbol(loc, field_sym))),
                    ],
                );
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Ast> {
        let loc = self.current_loc();

        if self.match_keyword("if") {
            return self.parse_if();
        }
        if self.match_keyword("match") {
            return self.parse_match();
        }
        if self.match_keyword("let") {
            return self.parse_let();
        }
        if self.match_keyword("do") {
            return self.parse_do_block();
        }
        if self.match_keyword("try") {
            return self.parse_try();
        }
        if self.match_keyword("yield") {
            self.advance();
            let val = self.parse_expr()?;
            return Ok(Ast::Yield(loc, Box::new(val)));
        }
        if self.peek().kind == TokenKind::BackSlash {
            return self.parse_lambda();
        }

        let tok = self.advance();
        match tok.kind {
            TokenKind::Number(base) => {
                let s = tok.source();
                if let Ok(i) = i64::from_str_radix(s, base.radix()) {
                    Ok(Ast::Integer(loc, i))
                } else if base == NumberBase::D && let Ok(f) = s.parse::<f64>() {
                    Ok(Ast::Float(loc, f))
                } else {
                    Err(SelError::SyntaxError(loc, format!("Invalid number: {s}")))
                }
            }
            TokenKind::RealNumber => {
                let s = tok.source();
                if let Ok(f) = s.parse::<f64>() {
                    Ok(Ast::Float(loc, f))
                } else {
                    Err(SelError::SyntaxError(loc, format!("Invalid float: {s}")))
                }
            }
            TokenKind::StringLiteral => Ok(Ast::String(loc, tok.unescape())),
            TokenKind::CharacterLiteral => {
                let s = tok.unescape();
                let c = s.chars().next().unwrap_or('\0');
                Ok(Ast::Char(loc, c))
            }
            TokenKind::Colon => {
                // Atom literal: `:ident` -> 'ident
                let ident_tok = self.expect_token(TokenKind::Identifier, "atom identifier after `:`")?;
                let sym_ast = Ast::Symbol(loc, intern(ident_tok.source()));
                Ok(Ast::Quote(loc, Box::new(sym_ast)))
            }
            TokenKind::OpenParen => {
                let expr = self.parse_expr()?;
                self.expect_token(TokenKind::CloseParen, "`)`")?;
                Ok(expr)
            }
            TokenKind::OpenBracket => self.parse_list_literal(loc),
            TokenKind::OpenCurly => self.parse_record_literal(loc),
            TokenKind::Keyword | TokenKind::Identifier => {
                match tok.source() {
                    "true" => Ok(Ast::Boolean(loc, true)),
                    "false" => Ok(Ast::Boolean(loc, false)),
                    "nil" => Ok(Ast::Nil(loc)),
                    name => Ok(Ast::Symbol(loc, intern(name))),
                }
            }
            _ => Err(SelError::SyntaxError(
                loc,
                format!("Unexpected token in expression: `{}`", tok.source()),
            )),
        }
    }

    fn parse_lambda(&mut self) -> Result<Ast> {
        let loc = self.current_loc();
        self.expect_token(TokenKind::BackSlash, "`\\` for lambda")?;

        let mut params = Vec::new();
        while self.peek().kind == TokenKind::Identifier && !KEYWORDS.contains(&self.peek().source()) {
            let p = self.advance();
            params.push(intern(p.source()));
        }

        self.expect_token(TokenKind::Arrow, "`->` after lambda parameters")?;
        let body = self.parse_expr()?;
        Ok(Ast::Lambda(loc, params, vec![body]))
    }

    fn parse_if(&mut self) -> Result<Ast> {
        let loc = self.current_loc();
        self.expect_keyword("if")?;
        let cond = self.parse_expr()?;
        self.expect_keyword("then")?;
        let then_branch = self.parse_expr()?;
        let else_branch = if self.eat_keyword("else") {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        Ok(Ast::If(loc, Box::new(cond), Box::new(then_branch), else_branch))
    }

    fn parse_let(&mut self) -> Result<Ast> {
        let loc = self.current_loc();
        self.expect_keyword("let")?;

        let mut bindings = Vec::new();
        loop {
            let var_tok = self.expect_token(TokenKind::Identifier, "variable name in let")?;
            let var_id = intern(var_tok.source());

            if self.peek().kind == TokenKind::Assign {
                self.advance();
            } else {
                self.expect_token(TokenKind::Eq, "`=` or `:=` in let binding")?;
            }

            let val = self.parse_expr()?;
            bindings.push((var_id, val));

            if self.peek().kind == TokenKind::Comma {
                self.advance();
            } else {
                break;
            }
        }

        self.expect_keyword("in")?;
        let body = self.parse_expr()?;
        Ok(Ast::Let(loc, bindings, vec![body]))
    }

    fn parse_do_block(&mut self) -> Result<Ast> {
        let loc = self.current_loc();
        self.expect_keyword("do")?;
        let mut exprs = Vec::new();
        self.skip_semicolons();

        while !self.match_keyword("end") && !self.at_eof() {
            let item = self.parse_item()?;
            exprs.push(item);
            self.skip_semicolons();
        }

        self.expect_keyword("end")?;
        Ok(Ast::Begin(loc, exprs))
    }

    fn parse_try(&mut self) -> Result<Ast> {
        let loc = self.current_loc();
        self.expect_keyword("try")?;
        let body = self.parse_expr()?;
        self.expect_keyword("catch")?;
        let err_tok = self.expect_token(TokenKind::Identifier, "error variable after catch")?;
        let err_id = intern(err_tok.source());
        self.expect_token(TokenKind::Arrow, "`->` after catch variable")?;
        let handler = self.parse_expr()?;
        Ok(Ast::Try(loc, Box::new(body), err_id, vec![handler]))
    }

    fn parse_list_literal(&mut self, loc: Loc) -> Result<Ast> {
        let mut elements = Vec::new();
        if self.peek().kind != TokenKind::CloseBracket {
            loop {
                elements.push(self.parse_expr()?);
                if self.peek().kind == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_token(TokenKind::CloseBracket, "`]`")?;

        let mut items = vec![Ast::Symbol(loc, intern("list"))];
        items.extend(elements);
        Ok(Ast::List(loc, items))
    }

    fn parse_record_literal(&mut self, loc: Loc) -> Result<Ast> {
        let mut fields = Vec::new();
        if self.peek().kind != TokenKind::CloseCurly {
            loop {
                let key_tok = if self.peek().kind == TokenKind::Colon {
                    self.advance();
                    self.expect_token(TokenKind::Identifier, "field name")?
                } else {
                    self.expect_token(TokenKind::Identifier, "field name")?
                };
                let key_id = intern(key_tok.source());

                let val = if self.peek().kind == TokenKind::Colon || self.peek().kind == TokenKind::Eq {
                    self.advance();
                    self.parse_expr()?
                } else if self.peek().kind == TokenKind::Comma || self.peek().kind == TokenKind::CloseCurly {
                    // Punned field: `{ x }` -> `{ x: x }`
                    Ast::Symbol(loc, key_id)
                } else {
                    self.parse_expr()?
                };

                fields.push((key_id, val));

                if self.peek().kind == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_token(TokenKind::CloseCurly, "`}`")?;
        Ok(Ast::Record(loc, fields))
    }

    fn parse_match(&mut self) -> Result<Ast> {
        let loc = self.current_loc();
        self.expect_keyword("match")?;
        let target = self.parse_expr()?;
        self.expect_keyword("with")?;

        let mut clauses = Vec::new();
        self.skip_semicolons();

        let mut first = true;
        while !self.at_eof() && !self.match_keyword("end") {
            let has_pipe = self.eat_token(TokenKind::Pipe);
            if !has_pipe && !first {
                break;
            }
            if !has_pipe && !self.is_pattern_start() {
                break;
            }
            first = false;

            let clause_loc = self.current_loc();
            let pat = self.parse_pattern()?;

            let guard = if self.eat_keyword("when") {
                Some(self.parse_expr()?)
            } else {
                None
            };

            self.expect_token(TokenKind::Arrow, "`->` after pattern")?;
            let body_expr = self.parse_expr()?;

            clauses.push(MatchClause {
                loc: clause_loc,
                pattern: pat,
                guard,
                body: vec![body_expr],
            });

            self.skip_semicolons();
        }

        self.eat_keyword("end");

        if clauses.is_empty() {
            return Err(SelError::SyntaxError(
                loc,
                "Expected at least one match clause in `match`".into(),
            ));
        }

        Ok(Ast::Match(loc, Box::new(target), clauses))
    }

    fn is_pattern_start(&self) -> bool {
        let tok = self.peek();
        matches!(
            tok.kind,
            TokenKind::Identifier
                | TokenKind::Number(_)
                | TokenKind::RealNumber
                | TokenKind::StringLiteral
                | TokenKind::CharacterLiteral
                | TokenKind::OpenBracket
                | TokenKind::OpenCurly
                | TokenKind::Colon
        )
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        let loc = self.current_loc();
        let tok = self.advance();

        match tok.kind {
            TokenKind::Identifier => {
                let is_escaped = self.pos > 0 && self.escaped_tokens.contains(&(self.pos - 1));
                if is_escaped {
                    let sym_ast = Ast::Symbol(loc, intern(tok.source()));
                    Ok(Pattern::Literal(loc, Box::new(Ast::Quote(loc, Box::new(sym_ast)))))
                } else if tok.source() == "_" {
                    Ok(Pattern::Wildcard(loc))
                } else if tok.source() == "true" {
                    Ok(Pattern::Literal(loc, Box::new(Ast::Boolean(loc, true))))
                } else if tok.source() == "false" {
                    Ok(Pattern::Literal(loc, Box::new(Ast::Boolean(loc, false))))
                } else if tok.source() == "nil" {
                    Ok(Pattern::Literal(loc, Box::new(Ast::Nil(loc))))
                } else {
                    Ok(Pattern::Variable(loc, intern(tok.source())))
                }
            }
            TokenKind::Colon => {
                let id_tok = self.expect_token(TokenKind::Identifier, "atom in pattern")?;
                let sym_ast = Ast::Symbol(loc, intern(id_tok.source()));
                Ok(Pattern::Literal(loc, Box::new(Ast::Quote(loc, Box::new(sym_ast)))))
            }
            TokenKind::Number(base) => {
                let s = tok.source();
                if let Ok(i) = i64::from_str_radix(s, base.radix()) {
                    Ok(Pattern::Literal(loc, Box::new(Ast::Integer(loc, i))))
                } else {
                    Err(SelError::SyntaxError(loc, format!("Invalid number in pattern: {s}")))
                }
            }
            TokenKind::RealNumber => {
                let s = tok.source();
                if let Ok(f) = s.parse::<f64>() {
                    Ok(Pattern::Literal(loc, Box::new(Ast::Float(loc, f))))
                } else {
                    Err(SelError::SyntaxError(loc, format!("Invalid float in pattern: {s}")))
                }
            }
            TokenKind::StringLiteral => {
                Ok(Pattern::Literal(loc, Box::new(Ast::String(loc, tok.unescape()))))
            }
            TokenKind::CharacterLiteral => {
                let s = tok.unescape();
                let c = s.chars().next().unwrap_or('\0');
                Ok(Pattern::Literal(loc, Box::new(Ast::Char(loc, c))))
            }
            TokenKind::OpenBracket => {
                // List pattern: `[]`, `[p1, p2]`, `[head | tail]`, `[p1, p2 | rest]`
                if self.peek().kind == TokenKind::CloseBracket {
                    self.advance();
                    return Ok(Pattern::List(loc, Vec::new()));
                }

                let mut prefix = Vec::new();
                let mut tail = None;

                loop {
                    prefix.push(self.parse_pattern()?);
                    if self.peek().kind == TokenKind::Pipe {
                        self.advance();
                        tail = Some(Box::new(self.parse_pattern()?));
                        break;
                    }
                    if self.peek().kind == TokenKind::Comma {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.expect_token(TokenKind::CloseBracket, "`]` in list pattern")?;

                if let Some(t) = tail {
                    if prefix.len() == 1 {
                        Ok(Pattern::Cons(loc, Box::new(prefix.pop().unwrap()), t))
                    } else {
                        Ok(Pattern::Rest(loc, prefix, t))
                    }
                } else {
                    Ok(Pattern::List(loc, prefix))
                }
            }
            TokenKind::OpenCurly => {
                // Record pattern: `{ key: pat, key2 }`
                let mut fields = Vec::new();
                if self.peek().kind != TokenKind::CloseCurly {
                    loop {
                        let key_tok = if self.peek().kind == TokenKind::Colon {
                            self.advance();
                            self.expect_token(TokenKind::Identifier, "field name in pattern")?
                        } else {
                            self.expect_token(TokenKind::Identifier, "field name in pattern")?
                        };
                        let key_id = intern(key_tok.source());

                        let pat = if self.peek().kind == TokenKind::Colon || self.peek().kind == TokenKind::Eq {
                            self.advance();
                            self.parse_pattern()?
                        } else {
                            // Punned: `{ x }` binds variable `x`
                            Pattern::Variable(loc, key_id)
                        };

                        fields.push((key_id, pat));

                        if self.peek().kind == TokenKind::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect_token(TokenKind::CloseCurly, "`}` in record pattern")?;
                Ok(Pattern::Record(loc, fields))
            }
            _ => Err(SelError::SyntaxError(
                loc,
                format!("Expected pattern, found `{}`", tok.source()),
            )),
        }
    }
}

pub fn parse_all(source: &str, file_id: u32, diags: &mut Vec<SelError>) -> Vec<Ast> {
    let mut parser = AltParser::new(source, file_id);
    parser.parse_program(diags)
}

/// Check if an alternative syntax source string represents a complete statement or expression.
/// Used by the REPL to determine whether to evaluate or continue reading multi-line input.
pub fn is_input_complete(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return true;
    }

    // 1. Check for unclosed literals, block comments, or escapes
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut in_escaped_ident = false;

    let mut paren_count: i32 = 0;
    let mut bracket_count: i32 = 0;
    let mut brace_count: i32 = 0;

    while i < len {
        let c = chars[i];

        if in_str {
            if c == '\\' && i + 1 < len {
                i += 2;
                continue;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            if c == '*' && i + 1 < len && chars[i + 1] == '/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if in_escaped_ident {
            if c == '\\' && i + 1 < len {
                i += 2;
                continue;
            } else if c == '\'' {
                in_escaped_ident = false;
            }
            i += 1;
            continue;
        }

        // Check comment starts
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            in_block_comment = true;
            i += 2;
            continue;
        }

        if c == '"' {
            in_str = true;
            i += 1;
            continue;
        }

        if c == ':' && i + 1 < len && chars[i + 1] == '\'' {
            in_escaped_ident = true;
            i += 2;
            continue;
        }

        match c {
            '(' => paren_count += 1,
            ')' => paren_count -= 1,
            '[' => bracket_count += 1,
            ']' => bracket_count -= 1,
            '{' => brace_count += 1,
            '}' => brace_count -= 1,
            _ => {}
        }

        i += 1;
    }

    if in_str || in_block_comment || in_escaped_ident {
        return false;
    }

    if paren_count > 0 || bracket_count > 0 || brace_count > 0 {
        return false;
    }

    // 2. Lex input to inspect block nesting (do...end, match...end) and trailing continuation tokens
    let (sanitized, _) = scan_escaped_identifiers(input);
    let mut lexer = Lexer::new(&sanitized).with_keywords(KEYWORDS);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next();
        if tok.is_eof() {
            break;
        }
        tokens.push(tok);
    }

    if tokens.is_empty() {
        return true;
    }

    // Track block depth
    let mut block_depth: i32 = 0;
    for tok in &tokens {
        if tok.kind == TokenKind::Keyword {
            match tok.source() {
                "do" | "match" => block_depth += 1,
                "end" => block_depth = (block_depth - 1).max(0),
                _ => {}
            }
        }
    }

    if block_depth > 0 {
        return false;
    }

    // Check trailing continuation tokens
    let last = tokens.last().unwrap();
    let second_to_last = if tokens.len() >= 2 {
        Some(&tokens[tokens.len() - 2])
    } else {
        None
    };

    // Trailing pipeline `|>`
    if last.kind == TokenKind::Gt && second_to_last.is_some_and(|t| t.kind == TokenKind::Pipe) {
        return false;
    }

    match last.kind {
        TokenKind::Assign
        | TokenKind::Arrow
        | TokenKind::Pipe
        | TokenKind::BackSlash
        | TokenKind::Comma
        | TokenKind::Dot
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Asterisk
        | TokenKind::Slash
        | TokenKind::Mod
        | TokenKind::EqEq
        | TokenKind::NotEq
        | TokenKind::Lt
        | TokenKind::LtEq
        | TokenKind::Gt
        | TokenKind::GtEq
        | TokenKind::DoubleAmpersand
        | TokenKind::DoublePipe => return false,
        TokenKind::Keyword => {
            match last.source() {
                "let" | "in" | "if" | "then" | "else" | "match" | "with" | "when"
                | "try" | "catch" | "pub" | "import" | "as" => return false,
                _ => {}
            }
        }
        _ => {}
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::load_core_lib;
    use crate::runtime::{execute_asts, Env};
    use crate::value::Value;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn eval_alt(source: &str) -> std::result::Result<Value, SelError> {
        let mut diags = Vec::new();
        let file_id = intern("<test.sel>");
        let asts = parse_all(source, file_id, &mut diags);
        if !diags.is_empty() {
            return Err(diags.remove(0));
        }
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());
        execute_asts(asts, env)
    }

    #[test]
    fn test_escaped_identifiers() {
        let code = r#"
            // Calling Scheme functions directly via escaped names
            t1 := :'type-of'(42)
            t2 := :'type-of'("hello")
            s := 99 |> :'to-string'

            // Defining a function with an escaped name
            :'add-and-double' x y := (x + y) * 2
            res := :'add-and-double'(3, 4)

            // Let binding with an escaped name
            let_res := let :'hyphen-var' = 50 in :'hyphen-var' + 10

            // Record with escaped field name
            rec := { :'user-id': 999 }
            field_val := rec.:'user-id'

            // Match pattern with escaped symbol literal
            matched := match quote(:'not-found') with
                | :'not-found' -> :ok
                | _ -> :err
            end

            [t1, t2, s, res, let_res, field_val, matched]
        "#;
        let val = eval_alt(code).expect("escaped identifiers evaluation failed");
        let s = format!("{val}");
        assert!(s.contains("int"));
        assert!(s.contains("string"));
        assert!(s.contains("99"));
        assert!(s.contains("14"));
        assert!(s.contains("60"));
        assert!(s.contains("999"));
        assert!(s.contains("ok"));
    }

    #[test]
    fn test_alt_is_input_complete() {
        assert!(is_input_complete("add x y := x + y"));
        assert!(!is_input_complete("add x y :="));
        assert!(!is_input_complete("10 |>"));
        assert!(!is_input_complete("do\n  x := 10"));
        assert!(is_input_complete("do\n  x := 10\nend"));
        assert!(!is_input_complete("match x with\n| :ok -> 1"));
        assert!(is_input_complete("match x with\n| :ok -> 1\nend"));
        assert!(!is_input_complete("let x = 10,"));
        assert!(!is_input_complete("let x = 10 in"));
        assert!(is_input_complete("let x = 10 in x + 1"));
        assert!(!is_input_complete("if x > 0 then"));
        assert!(!is_input_complete("if x > 0 then 1 else"));
        assert!(is_input_complete("if x > 0 then 1 else 2"));
        assert!(!is_input_complete("try error(\"fail\") catch e ->"));
        assert!(is_input_complete("try error(\"fail\") catch e -> 0"));
        assert!(!is_input_complete(":'type-"));
        assert!(is_input_complete(":'type-of'(42)"));
        assert!(!is_input_complete("/* unclosed"));
        assert!(is_input_complete("/* closed */ 42"));
        assert!(is_input_complete("// line comment\n42"));
    }

    #[test]
    fn test_variable_and_function_defs() {
        let code = r#"
            x := 40
            add a b := a + b
            add(x, 2)
        "#;
        let val = eval_alt(code).expect("evaluation failed");
        assert!(matches!(val, Value::Integer(42)));
    }

    #[test]
    fn test_lambda_and_pipeline() {
        let code = r#"
            double := \x -> x * 2
            sub a b := a - b
            10 |> sub(25) |> double
        "#;
        // 10 |> sub(25) -> sub(25, 10) = 15
        // 15 |> double -> double(15) = 30
        let val = eval_alt(code).expect("pipeline failed");
        assert!(matches!(val, Value::Integer(30)));
    }

    #[test]
    fn test_pipeline_and_equality_precedence() {
        let code = r#"
            sub a b := a - b
            10 |> sub(35) == 25
        "#;
        let val = eval_alt(code).expect("precedence failed");
        assert!(matches!(val, Value::Boolean(true)));
    }

    #[test]
    fn test_conditionals_and_let() {
        let code = r#"
            let a = 15, b = 25 in
                if a > b then a else b
        "#;
        let val = eval_alt(code).expect("let/if failed");
        assert!(matches!(val, Value::Integer(25)));
    }

    #[test]
    fn test_do_block() {
        let code = r#"
            do
                x := 10
                y := 20
                x * y
            end
        "#;
        let val = eval_alt(code).expect("do block failed");
        assert!(matches!(val, Value::Integer(200)));
    }

    #[test]
    fn test_record_literal_and_dot_access() {
        let code = r#"
            user := { id: 101, name: "Alice", active: true }
            user.name
        "#;
        let val = eval_alt(code).expect("record failed");
        assert_eq!(format!("{val}"), "Alice");
    }

    #[test]
    fn test_match_pattern() {
        let code = r#"
            classify status :=
                match status with
                | :ok -> "success"
                | :error -> "failure"
                | _ -> "unknown"

            classify(:ok)
        "#;
        let val = eval_alt(code).expect("match failed");
        assert_eq!(format!("{val}"), "success");
    }

    #[test]
    fn test_list_and_match_destructure() {
        let code = r#"
            sum_first_two list :=
                match list with
                | [a, b | rest] -> a + b
                | [a] -> a
                | [] -> 0

            sum_first_two([10, 20, 30, 40])
        "#;
        let val = eval_alt(code).expect("list match failed");
        assert!(matches!(val, Value::Integer(30)));
    }

    #[test]
    fn test_try_catch() {
        let code = r#"
            try
                error("boom")
            catch err ->
                999
        "#;
        let val = eval_alt(code).expect("try/catch failed");
        assert!(matches!(val, Value::Integer(999)));
    }

    #[test]
    fn test_pub_visibility_scoping() {
        let code = r#"
            pub exported_fn x := x + 10
            private_val := 42
            pub exported_val := 100
        "#;
        let mut diags = Vec::new();
        let file_id = intern("<vis_test.sel>");
        let asts = parse_all(code, file_id, &mut diags);
        assert!(diags.is_empty());
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());
        env.borrow_mut().current_visibility_public = false;
        execute_asts(asts, env.clone()).expect("exec failed");

        let borrowed = env.borrow();
        let exported_sym = intern("exported_fn");
        let private_sym = intern("private_val");
        let exported_val_sym = intern("exported_val");

        assert!(!borrowed.private_bindings.contains(&exported_sym));
        assert!(borrowed.private_bindings.contains(&private_sym));
        assert!(!borrowed.private_bindings.contains(&exported_val_sym));
    }
}
