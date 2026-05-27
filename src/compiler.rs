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

#[derive(Debug, Clone)]
pub struct Local {
    pub name: u32,
    pub depth: usize,
}

pub struct Compiler<'a> {
    pub chunk: &'a mut Chunk,
    pub locals: Vec<Local>,
    pub scope_depth: usize,
}

impl<'a> Compiler<'a> {
    pub fn new(chunk: &'a mut Chunk) -> Self {
        Self {
            chunk,
            locals: Vec::new(),
            scope_depth: 0,
        }
    }

    pub fn compile(&mut self, ast: Ast) -> Result<()> {
        self.compile_expr(ast, false)
    }

    pub fn compile_expr(&mut self, ast: Ast, is_tail: bool) -> Result<()> {
        match ast {
            Ast::Import(loc, id, alias) => {
                self.chunk.write((loc, OpCode::Import(id, alias)));
            }
            Ast::VisibilityDirective(loc, is_public) => {
                self.chunk.write((loc, OpCode::SetVisibility(is_public)));
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
            Ast::Char(loc, c) => {
                let idx = self.chunk.add_constant(Value::Char(c));
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Nil(loc) => {
                let idx = self.chunk.add_constant(Value::Nil);
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Symbol(loc, id) => {
                if let Some(index) = self.locals.iter().rposition(|local| local.name == id) {
                    self.chunk.write((loc, OpCode::LoadLocal(index as u8)));
                } else {
                    self.chunk.write((loc, OpCode::LoadVar(id)));
                }
            }
            Ast::Define(loc, id, expr) => {
                self.compile(*expr)?;
                self.chunk.write((loc, OpCode::DefVar(id)));
            }
            Ast::Set(loc, id, expr) => {
                self.compile(*expr)?;
                if let Some(index) = self.locals.iter().rposition(|local| local.name == id) {
                    self.chunk.write((loc, OpCode::StoreLocal(index as u8)));
                } else {
                    self.chunk.write((loc, OpCode::StoreVar(id)));
                }
            }
            Ast::Cond(loc, branches) => {
                let mut patches = Vec::new();
                for (c, e) in branches {
                    self.compile(c)?;
                    let jump_if_false_idx = self.chunk.code.len();
                    self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                    self.chunk.write((loc, OpCode::Pop));
                    self.compile_expr(e, is_tail)?;

                    patches.push(self.chunk.code.len());
                    self.chunk.write((loc, OpCode::Jump(0)));

                    let jump_end_idx = self.chunk.code.len();
                    self.chunk.write((loc, OpCode::Jump(0)));

                    self.chunk
                        .patch_jump(jump_if_false_idx, self.chunk.code.len());
                    self.chunk.write((loc, OpCode::Pop));

                    self.chunk.patch_jump(jump_end_idx, self.chunk.code.len());
                }
                for patch in patches {
                    self.chunk.patch_jump(patch, self.chunk.code.len());
                }
            }
            Ast::While(loc, cond, body) => {
                let cond_idx = self.chunk.code.len();
                self.compile(*cond)?;
                let jump_if_false_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                if let Ast::List(_, list) = *body {
                    for expr in list {
                        self.compile_expr(expr, is_tail)?;
                    }
                } else {
                    unreachable!("While body should be always a list.")
                }

                self.chunk.write((loc, OpCode::Jump(cond_idx)));

                self.chunk
                    .patch_jump(jump_if_false_idx, self.chunk.code.len());
                self.chunk.write((loc, OpCode::Pop));
            }
            Ast::Until(loc, cond, body) => {
                let cond_idx = self.chunk.code.len();
                self.compile(*cond)?;
                let jump_if_false_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                let jump_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::Jump(0)));

                self.chunk
                    .patch_jump(jump_if_false_idx, self.chunk.code.len());
                self.chunk.write((loc, OpCode::Pop));

                if let Ast::List(_, list) = *body {
                    for expr in list {
                        self.compile_expr(expr, is_tail)?;
                    }
                } else {
                    unreachable!("Until body should be always a list.")
                }

                self.chunk.write((loc, OpCode::Jump(cond_idx)));

                self.chunk.patch_jump(jump_idx, self.chunk.code.len());
                self.chunk.write((loc, OpCode::Pop));
            }
            Ast::If(loc, cond, true_branch, false_branch) => {
                self.compile(*cond)?;
                let jump_if_false_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                self.chunk.write((loc, OpCode::Pop));
                self.compile_expr(*true_branch, is_tail)?;

                let jump_end_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::Jump(0)));

                self.chunk
                    .patch_jump(jump_if_false_idx, self.chunk.code.len());
                self.chunk.write((loc, OpCode::Pop));

                if let Some(fb) = false_branch {
                    self.compile_expr(*fb, is_tail)?;
                } else {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                }

                self.chunk.patch_jump(jump_end_idx, self.chunk.code.len());
            }
            Ast::Unless(loc, cond, false_branch, true_branch) => {
                self.compile(*cond)?;
                let jump_if_false_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                self.chunk.write((loc, OpCode::Pop));
                if let Some(fb) = true_branch {
                    self.compile_expr(*fb, is_tail)?;
                } else {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                }

                let jump_end_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::Jump(0)));

                self.chunk
                    .patch_jump(jump_if_false_idx, self.chunk.code.len());
                self.chunk.write((loc, OpCode::Pop));

                self.compile_expr(*false_branch, is_tail)?;

                self.chunk.patch_jump(jump_end_idx, self.chunk.code.len());
            }
            Ast::When(loc, cond, mut body) => {
                self.compile(*cond)?;
                let jump_if_false_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::JumpIfFalse(0)));

                self.chunk.write((loc, OpCode::Pop));

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

                let jump_end_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::Jump(0)));

                self.chunk
                    .patch_jump(jump_if_false_idx, self.chunk.code.len());
                self.chunk.write((loc, OpCode::Pop));

                self.chunk.patch_jump(jump_end_idx, self.chunk.code.len());
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
                self.scope_depth += 1;
                let mut ids = Vec::new();
                for (id, val) in bindings {
                    self.compile(val)?;
                    ids.push(id);
                    self.locals.push(Local {
                        name: id,
                        depth: self.scope_depth,
                    });
                }
                self.chunk.write((loc, OpCode::BuildEnv(ids.clone())));

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

                let count = ids.len();
                self.chunk.write((loc, OpCode::PopEnv(count)));
                self.locals.retain(|local| local.depth < self.scope_depth);
                self.scope_depth -= 1;
            }
            Ast::Lambda(loc, params, mut body_asts) => {
                let mut child_chunk = Chunk::new();
                let mut child_compiler = Compiler::new(&mut child_chunk);
                for param_id in &params {
                    child_compiler.locals.push(Local {
                        name: *param_id,
                        depth: 0,
                    });
                }

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
                    for param_id in &params {
                        child_compiler.locals.push(Local {
                            name: *param_id,
                            depth: 0,
                        });
                    }

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
                self.chunk.patch_jump(catch_idx, catch_start_ip);

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

                self.chunk.write((loc, OpCode::PopEnv(1)));

                let end_ip = self.chunk.code.len();
                self.chunk.patch_jump(jump_end_idx, end_ip);
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
                    self.chunk.patch_jump(jmp, end_pos);
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

                        self.chunk.patch_jump(jmp_false, self.chunk.code.len());
                        self.chunk.write((loc, OpCode::Pop));
                    }
                }

                let end_pos = self.chunk.code.len();
                for jmp in jump_ends {
                    self.chunk.patch_jump(jmp, end_pos);
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
    LoadLocal(u8),
    StoreLocal(u8),
    Pop,
    JumpIfFalse(usize),
    Jump(usize),
    Call(usize),
    TailCall(usize),
    MakeClosure(usize),
    MakeMacro(u32, usize),
    Return,
    BuildEnv(Vec<u32>),
    PopEnv(usize),
    RegisterCatch(usize),
    UnregisterCatch,
    Yield,
    CoResume,
    Import(u32, Option<u32>),
    SetVisibility(bool),
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
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub locations: Vec<(usize, Loc)>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            locations: Vec::new(),
        }
    }

    pub fn write(&mut self, op_loc: OpCodeLoc) {
        let (loc, op) = op_loc;
        let start_offset = self.code.len();
        self.locations.push((start_offset, loc));

        match op {
            OpCode::Constant(idx) => {
                self.code.push(1);
                self.code.extend_from_slice(&(idx as u32).to_le_bytes());
            }
            OpCode::LoadVar(id) => {
                self.code.push(2);
                self.code.extend_from_slice(&id.to_le_bytes());
            }
            OpCode::StoreVar(id) => {
                self.code.push(3);
                self.code.extend_from_slice(&id.to_le_bytes());
            }
            OpCode::DefVar(id) => {
                self.code.push(4);
                self.code.extend_from_slice(&id.to_le_bytes());
            }
            OpCode::LoadLocal(idx) => {
                self.code.push(5);
                self.code.push(idx);
            }
            OpCode::StoreLocal(idx) => {
                self.code.push(6);
                self.code.push(idx);
            }
            OpCode::Pop => {
                self.code.push(7);
            }
            OpCode::JumpIfFalse(target) => {
                self.code.push(8);
                self.code.extend_from_slice(&(target as u32).to_le_bytes());
            }
            OpCode::Jump(target) => {
                self.code.push(9);
                self.code.extend_from_slice(&(target as u32).to_le_bytes());
            }
            OpCode::Call(arity) => {
                self.code.push(10);
                self.code.extend_from_slice(&(arity as u32).to_le_bytes());
            }
            OpCode::TailCall(arity) => {
                self.code.push(11);
                self.code.extend_from_slice(&(arity as u32).to_le_bytes());
            }
            OpCode::MakeClosure(idx) => {
                self.code.push(12);
                self.code.extend_from_slice(&(idx as u32).to_le_bytes());
            }
            OpCode::MakeMacro(id, idx) => {
                self.code.push(13);
                self.code.extend_from_slice(&id.to_le_bytes());
                self.code.extend_from_slice(&(idx as u32).to_le_bytes());
            }
            OpCode::Return => {
                self.code.push(14);
            }
            OpCode::BuildEnv(ids) => {
                self.code.push(15);
                self.code
                    .extend_from_slice(&(ids.len() as u32).to_le_bytes());
                for id in ids {
                    self.code.extend_from_slice(&id.to_le_bytes());
                }
            }
            OpCode::PopEnv(count) => {
                self.code.push(16);
                self.code.extend_from_slice(&(count as u32).to_le_bytes());
            }
            OpCode::RegisterCatch(target) => {
                self.code.push(17);
                self.code.extend_from_slice(&(target as u32).to_le_bytes());
            }
            OpCode::UnregisterCatch => {
                self.code.push(18);
            }
            OpCode::Yield => {
                self.code.push(19);
            }
            OpCode::CoResume => {
                self.code.push(20);
            }
            OpCode::Import(mod_name_id, alias_opt) => {
                self.code.push(21);
                self.code.extend_from_slice(&mod_name_id.to_le_bytes());
                match alias_opt {
                    None => {
                        self.code.push(0);
                    }
                    Some(alias_id) => {
                        self.code.push(1);
                        self.code.extend_from_slice(&alias_id.to_le_bytes());
                    }
                }
            }
            OpCode::SetVisibility(is_public) => {
                self.code.push(22);
                self.code.push(if is_public { 1 } else { 0 });
            }
            OpCode::MakeRecord => {
                self.code.push(23);
            }
            OpCode::AssocRecord(sym) => {
                self.code.push(24);
                self.code.extend_from_slice(&sym.to_le_bytes());
            }
            OpCode::MakeList(len) => {
                self.code.push(25);
                self.code.extend_from_slice(&(len as u32).to_le_bytes());
            }
            OpCode::ConcatList(count) => {
                self.code.push(26);
                self.code.extend_from_slice(&(count as u32).to_le_bytes());
            }
            OpCode::Sum(arity) => {
                self.code.push(27);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::Sub(arity) => {
                self.code.push(28);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::Mul(arity) => {
                self.code.push(29);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::Div(arity) => {
                self.code.push(30);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::Mod => {
                self.code.push(31);
            }
            OpCode::Eq(arity) => {
                self.code.push(32);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::NumEq(arity) => {
                self.code.push(33);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::NumNotEq(arity) => {
                self.code.push(34);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::NumLt(arity) => {
                self.code.push(35);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::NumGt(arity) => {
                self.code.push(36);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::NumLte(arity) => {
                self.code.push(37);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::NumGte(arity) => {
                self.code.push(38);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
            OpCode::Cons => {
                self.code.push(39);
            }
            OpCode::Car => {
                self.code.push(40);
            }
            OpCode::Cdr => {
                self.code.push(41);
            }
            OpCode::Nth => {
                self.code.push(42);
            }
            OpCode::Count => {
                self.code.push(43);
            }
            OpCode::Empty => {
                self.code.push(44);
            }
            OpCode::IsNil => {
                self.code.push(45);
            }
            OpCode::IsList => {
                self.code.push(46);
            }
            OpCode::IsNumber => {
                self.code.push(47);
            }
            OpCode::IsString => {
                self.code.push(48);
            }
            OpCode::IsSymbol => {
                self.code.push(49);
            }
            OpCode::IsFunction => {
                self.code.push(50);
            }
            OpCode::TypeOf => {
                self.code.push(51);
            }
            OpCode::Not(arity) => {
                self.code.push(52);
                self.code.extend_from_slice(&arity.to_le_bytes());
            }
        }
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    pub fn patch_jump(&mut self, offset: usize, target: usize) {
        let bytes = (target as u32).to_le_bytes();
        self.code[offset + 1..offset + 5].copy_from_slice(&bytes);
    }

    pub fn get_loc(&self, ip: usize) -> Loc {
        match self
            .locations
            .binary_search_by_key(&ip, |&(offset, _)| offset)
        {
            Ok(idx) => self.locations[idx].1,
            Err(idx) => {
                if idx > 0 {
                    self.locations[idx - 1].1
                } else {
                    Loc::default()
                }
            }
        }
    }
}
