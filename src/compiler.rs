use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::*;
use crate::diagnostics::*;
use crate::lexer::Loc;
use crate::runtime::*;
use crate::types::lookup;

use crate::value::Closure;
use crate::value::Macro;
use crate::value::Value;

type Result<T> = std::result::Result<T, SelError>;

pub struct Compiler<'a> {
    pub chunk: &'a mut Chunk,
}

impl<'a> Compiler<'a> {
    pub fn new(chunk: &'a mut Chunk) -> Self {
        Self { chunk }
    }

    pub fn compile(&mut self, ast: Ast) -> Result<()> {
        self.compile_expr(ast, false)
    }

    pub fn compile_expr(&mut self, ast: Ast, is_tail: bool) -> Result<()> {
        match ast {
            Ast::Import(loc, id, alias) => {
                self.chunk.write((loc, OpCode::Import(id, alias)));
            }
            Ast::Integer(loc, i) => {
                let idx = self.chunk.add_constant(Value::Integer(i));
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Float(loc, f) => {
                let idx = self.chunk.add_constant(Value::Float(f));
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::String(loc, s) => {
                let idx = self.chunk.add_constant(Value::String(Rc::new(s)));
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Boolean(loc, b) => {
                let idx = self.chunk.add_constant(Value::Boolean(b));
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Nil(loc) => {
                let idx = self.chunk.add_constant(Value::Nil);
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Symbol(loc, id) => {
                self.chunk.write((loc, OpCode::LoadVar(id)));
            }
            Ast::Define(loc, id, expr) => {
                self.compile(*expr)?;
                self.chunk.write((loc, OpCode::DefVar(id)));
            }
            Ast::Set(loc, id, expr) => {
                self.compile(*expr)?;
                self.chunk.write((loc, OpCode::StoreVar(id)));
            }
            Ast::If(loc, cond, true_branch, false_branch) => {
                self.compile(*cond)?;
                let jump_if_false_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                self.chunk.write((loc, OpCode::Pop));
                self.compile_expr(*true_branch, is_tail)?;

                let jump_end_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::Jump(0)));

                self.chunk.code[jump_if_false_idx] =
                    (loc, OpCode::JumpIfFalse(self.chunk.code.len()));
                self.chunk.write((loc, OpCode::Pop));

                if let Some(fb) = false_branch {
                    self.compile_expr(*fb, is_tail)?;
                } else {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                }

                self.chunk.code[jump_end_idx] = (loc, OpCode::Jump(self.chunk.code.len()));
            }
            Ast::Begin(loc, mut exprs) => {
                if exprs.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                } else {
                    let last = exprs.pop().unwrap();
                    for expr in exprs {
                        self.compile(expr)?;
                        self.chunk.write((loc, OpCode::Pop));
                    }
                    self.compile_expr(last, is_tail)?;
                }
            }
            Ast::Let(loc, bindings, mut body) => {
                let mut ids = Vec::new();
                for (id, val) in bindings {
                    self.compile(val)?;
                    ids.push(id);
                }
                self.chunk.write((loc, OpCode::BuildEnv(ids)));

                if body.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                } else {
                    let last = body.pop().unwrap();
                    for expr in body {
                        self.compile(expr)?;
                        self.chunk.write((loc, OpCode::Pop));
                    }
                    self.compile_expr(last, is_tail)?;
                }

                self.chunk.write((loc, OpCode::PopEnv));
            }
            Ast::Lambda(loc, params, mut body_asts) => {
                let mut child_chunk = Chunk::new();
                let mut child_compiler = Compiler::new(&mut child_chunk);

                if body_asts.is_empty() {
                    let idx = child_chunk.add_constant(Value::Nil);
                    child_chunk.write((loc, OpCode::Constant(idx)));
                } else {
                    let last = body_asts.pop().unwrap();
                    for expr in body_asts {
                        child_compiler.compile(expr)?;
                        child_compiler.chunk.write((loc, OpCode::Pop));
                    }
                    child_compiler.compile_expr(last, true)?;
                }
                child_chunk.write((loc, OpCode::Return));

                let stub = Value::Closure(Rc::new(Closure {
                    params,
                    chunk: Rc::new(child_chunk),
                    env: Rc::new(RefCell::new(Env::default())),
                }));
                let idx = self.chunk.add_constant(stub);
                self.chunk.write((loc, OpCode::MakeClosure(idx)));
            }
            Ast::DefMacro(loc, id, expr) => {
                // Compile the macro body as a lambda, then make it a macro
                if let Ast::Lambda(_, params, mut body_asts) = *expr {
                    let mut child_chunk = Chunk::new();
                    let mut child_compiler = Compiler::new(&mut child_chunk);

                    if body_asts.is_empty() {
                        let idx = child_chunk.add_constant(Value::Nil);
                        child_chunk.write((loc, OpCode::Constant(idx)));
                    } else {
                        let last = body_asts.pop().unwrap();
                        for expr in body_asts {
                            child_compiler.compile(expr)?;
                            child_compiler.chunk.write((loc, OpCode::Pop));
                        }
                        child_compiler.compile_expr(last, true)?;
                    }
                    child_chunk.write((loc, OpCode::Return));

                    let stub = Value::Macro(Rc::new(Macro {
                        params,
                        chunk: Rc::new(child_chunk),
                        env: Rc::new(RefCell::new(Env::default())),
                    }));
                    let idx = self.chunk.add_constant(stub);
                    self.chunk.write((loc, OpCode::MakeMacro(id, idx)));
                } else {
                    return Err(SelError::SyntaxError(
                        loc,
                        "defmacro expects a lambda".into(),
                    ));
                }
            }
            Ast::Try(loc, body, err_var, catch_body) => {
                let catch_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::RegisterCatch(0)));

                // Compile the try body. It is NOT in tail position because we must run UnregisterCatch afterwards!
                self.compile_expr(*body, false)?;

                self.chunk.write((loc, OpCode::UnregisterCatch));

                let jump_end_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::Jump(0)));

                // This is the start of the catch block
                let catch_start_ip = self.chunk.code.len();
                self.chunk.code[catch_idx] = (loc, OpCode::RegisterCatch(catch_start_ip));

                // We build an environment for the error variable (which the VM will have pushed onto the stack)
                self.chunk.write((loc, OpCode::BuildEnv(vec![err_var])));

                if catch_body.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                } else {
                    let mut iter = catch_body.into_iter();
                    let last = iter.next_back().unwrap();
                    for expr in iter {
                        self.compile(expr)?;
                        self.chunk.write((loc, OpCode::Pop));
                    }
                    self.compile_expr(last, is_tail)?;
                }

                self.chunk.write((loc, OpCode::PopEnv));

                let end_ip = self.chunk.code.len();
                self.chunk.code[jump_end_idx] = (loc, OpCode::Jump(end_ip));
            }
            Ast::Yield(loc, val) => {
                self.compile(*val)?;
                self.chunk.write((loc, OpCode::Yield));
            }
            Ast::CoResume(loc, co, arg) => {
                self.compile(*co)?;
                self.compile(*arg)?;
                self.chunk.write((loc, OpCode::CoResume));
            }
            Ast::List(loc, list) => {
                if list.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                    return Ok(());
                }

                let mut iter = list.into_iter();
                let next = iter.next();
                if let Some(Ast::Symbol(loc, sym)) = next {
                    match lookup(sym).as_str() {
                        "eq?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            self.chunk.write((loc, OpCode::Eq(arg_count)));
                            return Ok(());
                        }
                        "mod" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected 2 arguments for mod".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Mod));
                            return Ok(());
                        }
                        "/" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count == 0 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 1 argument to /".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Div(arg_count)));
                            return Ok(());
                        }
                        "-" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count < 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 1 argument for -".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Sub(arg_count)));
                            return Ok(());
                        }
                        "+" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            self.chunk.write((loc, OpCode::Sum(arg_count)));
                            return Ok(());
                        }
                        "*" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            self.chunk.write((loc, OpCode::Mul(arg_count)));
                            return Ok(());
                        }
                        "=" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count < 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 2 arguments for =".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::NumEq(arg_count)));
                            return Ok(());
                        }
                        "!=" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count < 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 2 arguments for !=".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::NumNotEq(arg_count)));
                            return Ok(());
                        }
                        "<" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count < 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 2 arguments for <".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::NumLt(arg_count)));
                            return Ok(());
                        }
                        ">" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count < 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 2 arguments for >".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::NumGt(arg_count)));
                            return Ok(());
                        }
                        "<=" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count < 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 2 arguments for <=".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::NumLte(arg_count)));
                            return Ok(());
                        }
                        ">=" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count < 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected at least 2 arguments for >=".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::NumGte(arg_count)));
                            return Ok(());
                        }
                        "cons" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 2 arguments for cons".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Cons));
                            return Ok(());
                        }
                        "car" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for car".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Car));
                            return Ok(());
                        }
                        "cdr" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for cdr".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Cdr));
                            return Ok(());
                        }
                        "nth" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 2 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 2 arguments for nth".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Nth));
                            return Ok(());
                        }
                        "count" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for count".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Count));
                            return Ok(());
                        }
                        "list" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            self.chunk.write((loc, OpCode::MakeList(arg_count)));
                            return Ok(());
                        }
                        "empty?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for empty?".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Empty));
                            return Ok(());
                        }
                        "nil?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for nil?".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::IsNil));
                            return Ok(());
                        }
                        "list?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for list?".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::IsList));
                            return Ok(());
                        }
                        "number?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for number?".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::IsNumber));
                            return Ok(());
                        }
                        "string?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for string?".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::IsString));
                            return Ok(());
                        }
                        "symbol?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for symbol?".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::IsSymbol));
                            return Ok(());
                        }
                        "function?" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for function?".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::IsFunction));
                            return Ok(());
                        }
                        "type-of" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for type-of".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::TypeOf));
                            return Ok(());
                        }
                        "not" => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            if arg_count != 1 {
                                return Err(SelError::SyntaxError(
                                    loc,
                                    "Expected exactly 1 arguments for not".into(),
                                ));
                            }
                            self.chunk.write((loc, OpCode::Not(arg_count)));
                            return Ok(());
                        }
                        _ => self.compile(Ast::Symbol(loc, sym))?,
                    }
                } else if let Some(next) = next {
                    self.compile(next)?;
                }

                let mut arg_count = 0;
                for arg in iter {
                    self.compile(arg)?;
                    arg_count += 1;
                }
                if is_tail {
                    self.chunk.write((loc, OpCode::TailCall(arg_count)));
                } else {
                    self.chunk.write((loc, OpCode::Call(arg_count)));
                }
            }
            Ast::Record(loc, record) => {
                self.chunk.write((loc, OpCode::MakeRecord));
                for (sym, arg) in record {
                    self.compile(arg)?;
                    self.chunk.write((loc, OpCode::AssocRecord(sym)));
                }
            }
            Ast::Quote(loc, expr) => {
                let val = ast_to_value(*expr).1;
                let idx = self.chunk.add_constant(val);
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Quasiquote(_, expr) => {
                self.compile_quasiquote(*expr)?;
            }
            Ast::And(loc, exprs) => {
                if exprs.is_empty() {
                    let idx = self.chunk.add_constant(Value::Boolean(true));
                    self.chunk.write((loc, OpCode::Constant(idx)));
                    return Ok(());
                }
                let mut jump_ends = Vec::new();

                for (i, expr) in exprs.iter().enumerate() {
                    let next_is_tail = if i == exprs.len() - 1 { is_tail } else { false };
                    self.compile_expr(expr.clone(), next_is_tail)?;
                    if i < exprs.len() - 1 {
                        let jmp_false = self.chunk.code.len();
                        self.chunk.write((loc, OpCode::JumpIfFalse(0)));
                        self.chunk.write((loc, OpCode::Pop));
                        jump_ends.push(jmp_false);
                    }
                }

                let end_pos = self.chunk.code.len();
                for jmp in jump_ends {
                    self.chunk.code[jmp] = (loc, OpCode::JumpIfFalse(end_pos));
                }
            }
            Ast::Or(loc, exprs) => {
                if exprs.is_empty() {
                    let idx = self.chunk.add_constant(Value::Boolean(false));
                    self.chunk.write((loc, OpCode::Constant(idx)));
                    return Ok(());
                }

                let mut jump_ends = Vec::new();
                for (i, expr) in exprs.iter().enumerate() {
                    let next_is_tail = if i == exprs.len() - 1 { is_tail } else { false };
                    self.compile_expr(expr.clone(), next_is_tail)?;
                    if i < exprs.len() - 1 {
                        let jmp_false = self.chunk.code.len();
                        self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                        let jmp_end = self.chunk.code.len();
                        self.chunk.write((loc, OpCode::Jump(0)));
                        jump_ends.push(jmp_end);

                        self.chunk.code[jmp_false] =
                            (loc, OpCode::JumpIfFalse(self.chunk.code.len()));
                        self.chunk.write((loc, OpCode::Pop));
                    }
                }

                let end_pos = self.chunk.code.len();
                for jmp in jump_ends {
                    self.chunk.code[jmp] = (loc, OpCode::Jump(end_pos));
                }
            }
            Ast::Unquote(loc, _) | Ast::UnquoteSplicing(loc, _) => {
                return Err(SelError::SyntaxError(
                    loc,
                    "unquote/unquote-splicing outside of quasiquote".into(),
                ));
            }
            Ast::Bind(loc, _) => {
                return Err(SelError::SyntaxError(
                    loc,
                    "unexpected & binding in normal expression".into(),
                ));
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
                return Err(SelError::SyntaxError(
                    loc,
                    "unquote-splicing invalid at top level of quasiquote".into(),
                ));
            }
            Ast::List(loc, list) => {
                let mut parts = 0;
                for item in list {
                    if let Ast::UnquoteSplicing(_, inner) = item {
                        self.compile(*inner)?;
                    } else {
                        self.compile_quasiquote(item)?;
                        self.chunk.write((loc, OpCode::MakeList(1)));
                    }
                    parts += 1;
                }
                if parts == 0 {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                } else {
                    self.chunk.write((loc, OpCode::ConcatList(parts)));
                }
            }
            Ast::Record(loc, record) => {
                self.chunk.write((loc, OpCode::MakeRecord));
                for (sym, arg) in record {
                    self.compile_quasiquote(arg)?;
                    self.chunk.write((loc, OpCode::AssocRecord(sym)));
                }
            }
            _ => {
                let (loc, val) = ast_to_value(ast);
                let idx = self.chunk.add_constant(val);
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
        }
        Ok(())
    }
}

pub type OpCodeLoc = (Loc, OpCode);

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
    TailCall(usize),
    MakeClosure(usize),
    MakeMacro(u32, usize),
    Return,
    BuildEnv(Vec<u32>),
    PopEnv,
    RegisterCatch(usize),
    UnregisterCatch,
    Yield,
    CoResume,
    Import(u32, Option<u32>),
    MakeRecord,
    AssocRecord(u32),
    MakeList(usize),
    ConcatList(usize),
    Sum(u32),
    Sub(u32),
    Mul(u32),
    Div(u32),
    Mod,
    Eq(u32),
    NumEq(u32),
    NumNotEq(u32),
    NumLt(u32),
    NumGt(u32),
    NumLte(u32),
    NumGte(u32),
    Cons,
    Car,
    Cdr,
    Nth,
    Count,
    Empty,
    IsNil,
    IsList,
    IsNumber,
    IsString,
    IsSymbol,
    IsFunction,
    TypeOf,
    Not(u32),
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<OpCodeLoc>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
        }
    }

    pub fn write(&mut self, op: OpCodeLoc) {
        self.code.push(op);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}
