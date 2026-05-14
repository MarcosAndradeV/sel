use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::*;
use crate::diagnostics::*;
use crate::lexer::Lexer;
use crate::lexer::Loc;
use crate::runtime::*;
use crate::types::has_symbol;
use crate::types::lookup;

type Result<T> = std::result::Result<T, SelError>;

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Symbol(u32),
    List(Vec<Value>),
    NativeFunction(fn(Vec<Value>, Rc<RefCell<Env>>) -> Result<Value>),
    Closure {
        params: Vec<u32>,
        chunk: Rc<Chunk>,
        env: Rc<RefCell<Env>>,
    },
    Macro {
        params: Vec<u32>,
        chunk: Rc<Chunk>,
        env: Rc<RefCell<Env>>,
    },
    Pointer(usize),
    Library(Rc<libloading::Library>),
}

fn format_value(val: &Value) -> String {
    match val {
        Value::Nil => "()".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => s.to_string(),
        Value::Boolean(b) => {
            if *b {
                "#t".to_string()
            } else {
                "#f".to_string()
            }
        }
        Value::Symbol(id) => lookup(*id),
        Value::List(l) => {
            let mut s = String::from("(");
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(&format_value(v));
            }
            s.push(')');
            s
        }
        Value::NativeFunction(_) => "<native function>".to_string(),
        Value::Closure { .. } => "<closure>".to_string(),
        Value::Macro { .. } => "<macro>".to_string(),
        Value::Pointer(p) => format!("<pointer: {:#x}>", p),
        Value::Library(_) => "<library>".to_string(),
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_value(self))
    }
}

pub struct Compiler<'a> {
    pub chunk: &'a mut Chunk,
}

impl<'a> Compiler<'a> {
    pub fn new(chunk: &'a mut Chunk) -> Self {
        Self { chunk }
    }

    pub fn compile(&mut self, ast: Ast) -> Result<()> {
        match ast {
            Ast::Integer(loc, i) => {
                let idx = self.chunk.add_constant(Value::Integer(i));
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::Float(loc, f) => {
                let idx = self.chunk.add_constant(Value::Float(f));
                self.chunk.write((loc, OpCode::Constant(idx)));
            }
            Ast::String(loc, s) => {
                let idx = self.chunk.add_constant(Value::String(s));
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
                self.compile(*true_branch)?;

                let jump_end_idx = self.chunk.code.len();
                self.chunk.write((loc, OpCode::Jump(0)));

                self.chunk.code[jump_if_false_idx] =
                    (loc, OpCode::JumpIfFalse(self.chunk.code.len()));
                self.chunk.write((loc, OpCode::Pop));

                if let Some(fb) = false_branch {
                    self.compile(*fb)?;
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
                    self.compile(last)?;
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
                    self.compile(last)?;
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
                    child_compiler.compile(last)?;
                }
                child_chunk.write((loc, OpCode::Return));

                let stub = Value::Closure {
                    params,
                    chunk: Rc::new(child_chunk),
                    env: Rc::new(RefCell::new(Env::default())),
                };
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
                        child_compiler.compile(last)?;
                    }
                    child_chunk.write((loc, OpCode::Return));

                    let stub = Value::Macro {
                        params,
                        chunk: Rc::new(child_chunk),
                        env: Rc::new(RefCell::new(Env::default())),
                    };
                    let idx = self.chunk.add_constant(stub);
                    self.chunk.write((loc, OpCode::MakeMacro(id, idx)));
                } else {
                    return Err(SelError::SyntaxError(
                        loc,
                        "defmacro expects a lambda".into(),
                    ));
                }
            }
            Ast::List(loc, list) => {
                if list.is_empty() {
                    let idx = self.chunk.add_constant(Value::Nil);
                    self.chunk.write((loc, OpCode::Constant(idx)));
                    return Ok(());
                }

                let mut iter = list.into_iter();
                if let Some(next) = iter.next() {
                    match next {
                        Ast::Symbol(loc, sym) if has_symbol(sym, "eq?") => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            self.chunk.write((loc, OpCode::Eq(arg_count)));
                            return Ok(());
                        }
                        Ast::Symbol(loc, sym) if has_symbol(sym, "mod") => {
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
                            self.chunk.write((loc, OpCode::Mod(arg_count)));
                            return Ok(());
                        }
                        Ast::Symbol(loc, sym) if has_symbol(sym, "/") => {
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
                        Ast::Symbol(loc, sym) if has_symbol(sym, "-") => {
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
                        Ast::Symbol(loc, sym) if has_symbol(sym, "+") => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            self.chunk.write((loc, OpCode::Sum(arg_count)));
                            return Ok(());
                        }
                        Ast::Symbol(loc, sym) if has_symbol(sym, "*") => {
                            let mut arg_count = 0;
                            for arg in iter {
                                self.compile(arg)?;
                                arg_count += 1;
                            }
                            self.chunk.write((loc, OpCode::Mul(arg_count)));
                            return Ok(());
                        }
                        _ => self.compile(next)?,
                    }
                }

                let mut arg_count = 0;
                for arg in iter {
                    self.compile(arg)?;
                    arg_count += 1;
                }
                self.chunk.write((loc, OpCode::Call(arg_count)));
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
                    self.compile(expr.clone())?;
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
                    self.compile(expr.clone())?;
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
    MakeClosure(usize),
    MakeMacro(u32, usize),
    Return,
    BuildEnv(Vec<u32>),
    PopEnv,
    MakeList(usize),
    ConcatList(usize),
    Sum(u32),
    Sub(u32),
    Mul(u32),
    Div(u32),
    Mod(u32),
    Eq(u32),
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

pub fn read_all(line: &str, file_id: u32) -> Result<Vec<Ast>> {
    let mut lex = Lexer::new(line, file_id);
    let mut tokens = Vec::new();
    while let Some(t) = lex.next_token()? {
        tokens.push(t);
    }
    let mut pos = 0;
    let mut asts = Vec::new();
    while pos < tokens.len() {
        asts.push(parse_expr(&tokens, &mut pos)?);
    }
    Ok(asts)
}
