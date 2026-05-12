use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::SYMBOLS;
use crate::ast::*;
use crate::compiler::*;
use crate::diagnostics::*;
use crate::lexer::*;

type Result<T> = std::result::Result<T, SelError>;

#[derive(Default)]
pub struct SymbolTable {
    map: HashMap<String, u32>,
    vec: Vec<String>,
}

impl SymbolTable {
    fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.map.get(name) {
            id
        } else {
            let id = self.vec.len() as u32;
            self.map.insert(name.to_string(), id);
            self.vec.push(name.to_string());
            id
        }
    }

    fn lookup(&self, id: u32) -> String {
        self.vec
            .get(id as usize)
            .cloned()
            .unwrap_or_else(|| format!("id:{}", id))
    }
}

pub fn intern(name: &str) -> u32 {
    SYMBOLS.with(|s| s.borrow_mut().intern(name))
}

pub fn lookup(id: u32) -> String {
    SYMBOLS.with(|s| s.borrow().lookup(id))
}

#[derive(Debug, Default)]
pub struct Env {
    pub bindings: HashMap<u32, Value>,
    pub parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Self {
            bindings: HashMap::new(),
            parent,
        }
    }

    fn get(&self, id: u32) -> Option<Value> {
        if let Some(val) = self.bindings.get(&id) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().get(id)
        } else {
            None
        }
    }

    pub fn insert(&mut self, id: u32, val: Value) {
        self.bindings.insert(id, val);
    }

    fn set(&mut self, id: u32, val: Value) -> bool {
        if let std::collections::hash_map::Entry::Occupied(mut e) = self.bindings.entry(id) {
            e.insert(val);
            true
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().set(id, val)
        } else {
            false
        }
    }
}
pub struct CallFrame {
    pub chunk: Rc<Chunk>,
    pub ip: usize,
    pub env: Rc<RefCell<Env>>,
}

pub struct VM {
    pub stack: Vec<Value>,
}

impl VM {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn run(&mut self, chunk: Rc<Chunk>, env: Rc<RefCell<Env>>) -> Result<Value> {
        let mut frames = vec![CallFrame { chunk, ip: 0, env }];

        loop {
            let frame_idx = frames.len() - 1;
            let frame = &mut frames[frame_idx];

            if frame.ip >= frame.chunk.code.len() {
                // End of root chunk
                if frames.len() == 1 {
                    return Ok(self.stack.pop().unwrap_or(Value::Nil));
                } else {
                    return Err(SelError {
                        loc: Loc::default(),
                        kind: SelErrorKind::Generic(
                            "Unexpected end of function bytecode".into(),
                        ),
                    });
                }
            }

            let (loc, instruction) = frame.chunk.code[frame.ip].clone();
            frame.ip += 1;

            match instruction {
                OpCode::Constant(idx) => {
                    self.stack.push(frame.chunk.constants[idx].clone());
                }
                OpCode::LoadVar(id) => {
                    if let Some(val) = frame.env.borrow().get(id) {
                        self.stack.push(val);
                    } else {
                        return Err(SelError {
                            loc,
                            kind: SelErrorKind::UndefinedVariable(id),
                        });
                    }
                }
                OpCode::StoreVar(id) => {
                    let val = self.stack.last().unwrap().clone();
                    if !frame.env.borrow_mut().set(id, val) {
                        return Err(SelError {
                            loc,
                            kind: SelErrorKind::UnboundVariable(id),
                        });
                    }
                }
                OpCode::DefVar(id) => {
                    let val = self.stack.pop().unwrap();
                    frame.env.borrow_mut().insert(id, val);
                    self.stack.push(Value::Symbol(id)); // define returns symbol
                }
                OpCode::Pop => {
                    self.stack.pop();
                }
                OpCode::JumpIfFalse(offset) => {
                    let val = self.stack.last().unwrap();
                    let is_false = matches!(val, Value::Boolean(false));
                    if is_false {
                        frame.ip = offset;
                    }
                }
                OpCode::Jump(offset) => {
                    frame.ip = offset;
                }
                OpCode::Call(arg_count) => {
                    let mut next_ip = frame.ip;
                    let mut is_tail_call = false;
                    let mut jumps = 0;
                    while next_ip < frame.chunk.code.len() && jumps < 10 {
                        match frame.chunk.code[next_ip].1 {
                            OpCode::Return => {
                                is_tail_call = true;
                                break;
                            }
                            OpCode::Jump(target) => {
                                next_ip = target;
                                jumps += 1;
                            }
                            OpCode::PopEnv => {
                                next_ip += 1;
                            }
                            _ => {
                                break;
                            }
                        }
                    }
                    let callee = self.stack[self.stack.len() - arg_count - 1].clone();
                    match callee {
                        Value::Closure {
                            params,
                            chunk,
                            env: c_env,
                        } => {
                            let mut call_env = Env::new(Some(c_env));
                            let mut has_rest = false;
                            for (i, id) in params.iter().enumerate() {
                                if lookup(*id).starts_with('&') {
                                    let rest_args = self
                                        .stack
                                        .split_off(self.stack.len() - (arg_count - i));
                                    let name = &lookup(*id)[1..];
                                    call_env.insert(intern(name), Value::List(rest_args));
                                    has_rest = true;
                                    break;
                                } else {
                                    if params.len() != arg_count {
                                        return Err(SelError {
                                            loc,
                                            kind: SelErrorKind::ArityMismatch {
                                                expected: params.len(),
                                                actual: arg_count,
                                            },
                                        });
                                    }
                                    let arg_idx = self.stack.len() - arg_count + i;
                                    call_env.insert(*id, self.stack[arg_idx].clone());
                                }
                            }
                            if !has_rest {
                                self.stack.truncate(self.stack.len() - arg_count);
                            }
                            self.stack.pop(); // pop callee
                            if is_tail_call {
                                frame.chunk = chunk;
                                frame.ip = 0;
                                frame.env = Rc::new(RefCell::new(call_env));
                            } else {
                                frames.push(CallFrame {
                                    chunk,
                                    ip: 0,
                                    env: Rc::new(RefCell::new(call_env)),
                                });
                            }
                        }
                        Value::NativeFunction(f) => {
                            let mut args = Vec::with_capacity(arg_count);
                            let start = self.stack.len() - arg_count;
                            args.extend(self.stack.drain(start..));
                            self.stack.pop(); // pop callee

                            let result = f(args, frame.env.clone())?;
                            self.stack.push(result);
                        }
                        Value::Macro { .. } => {
                            return Err(SelError {
                                loc,
                                kind: SelErrorKind::Generic(
                                    "Cannot call macro at runtime".into(),
                                ),
                            });
                        }
                        _ => {
                            return Err(SelError {
                                loc,
                                kind: SelErrorKind::Generic(format!(
                                    "Attempt to call non-function value: {}",
                                    callee
                                )),
                            });
                        }
                    }
                }
                OpCode::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    frames.pop();
                    self.stack.push(result);
                    if frames.is_empty() {
                        return Ok(self.stack.pop().unwrap());
                    }
                }
                OpCode::BuildEnv(ids) => {
                    let mut let_env = Env::new(Some(frame.env.clone()));
                    let start = self.stack.len() - ids.len();
                    let vals: Vec<Value> = self.stack.drain(start..).collect();
                    for (id, val) in ids.into_iter().zip(vals) {
                        let_env.insert(id, val);
                    }
                    frame.env = Rc::new(RefCell::new(let_env));
                }
                OpCode::PopEnv => {
                    let parent = frame.env.borrow().parent.clone().unwrap();
                    frame.env = parent;
                }
                OpCode::MakeClosure(idx) => {
                    if let Value::Closure { params, chunk, .. } =
                        frame.chunk.constants[idx].clone()
                    {
                        let closure = Value::Closure {
                            params,
                            chunk,
                            env: frame.env.clone(),
                        };
                        self.stack.push(closure);
                    }
                }
                OpCode::MakeMacro(id, idx) => {
                    if let Value::Macro { params, chunk, .. } =
                        frame.chunk.constants[idx].clone()
                    {
                        let mac = Value::Macro {
                            params,
                            chunk,
                            env: frame.env.clone(),
                        };
                        frame.env.borrow_mut().insert(id, mac.clone());
                        self.stack.push(Value::Symbol(id));
                    }
                }
                OpCode::MakeList(count) => {
                    let mut items = Vec::with_capacity(count);
                    let start = self.stack.len() - count;
                    items.extend(self.stack.drain(start..));
                    self.stack.push(Value::List(items));
                }
                OpCode::ConcatList(count) => {
                    let mut items = Vec::new();
                    let start = self.stack.len() - count;
                    for val in self.stack.drain(start..) {
                        match val {
                            Value::List(l) => items.extend(l),
                            Value::Nil => {}
                            _ => {
                                return Err(SelError {
                                    loc,
                                    kind: SelErrorKind::Generic(
                                        "unquote-splicing requires a list".into(),
                                    ),
                                });
                            }
                        }
                    }
                    self.stack.push(Value::List(items));
                }
            }
        }
    }
}

pub fn macro_expand(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Ast> {
    match ast {
        Ast::List(loc, list) => {
            if list.is_empty() {
                return Ok(Ast::List(loc, list));
            }
            if let Ast::Symbol(loc, id) = list[0].clone() {
                let macro_opt = env.borrow().get(id);
                if let Some(Value::Macro {
                    params,
                    chunk,
                    env: m_env,
                }) = macro_opt
                {
                    let mut list_iter = list.into_iter();
                    list_iter.next(); // skip head
                    let args_ast: Vec<Ast> = list_iter.collect();
                    let mut args: Vec<Value> = Vec::new();
                    for a in args_ast {
                        args.push(ast_to_value(a).1);
                    }

                    let mut call_env = Env::new(Some(m_env));

                    for (i, pid) in params.iter().enumerate() {
                        if lookup(*pid).starts_with('&') {
                            let rest_args = args.split_off(i);
                            let name = &lookup(*pid)[1..];
                            call_env.insert(intern(name), Value::List(rest_args));
                            break;
                        } else if i < args.len() {
                            call_env.insert(*pid, args[i].clone());
                        } else {
                            return Err(SelError {
                                loc,
                                kind: SelErrorKind::Generic(
                                    "Arity mismatch in macro call".into(),
                                ),
                            });
                        }
                    }

                    let mut vm = VM::new();
                    let result_val = vm.run(chunk, Rc::new(RefCell::new(call_env)))?;

                    let expanded_ast = value_to_ast(result_val, loc)?;
                    return macro_expand(expanded_ast, env);
                }
            }

            let mut expanded_list = Vec::new();
            for item in list {
                expanded_list.push(macro_expand(item, env.clone())?);
            }
            Ok(Ast::List(loc, expanded_list))
        }
        Ast::Begin(loc, exprs) => {
            let mut exp = Vec::new();
            for e in exprs {
                exp.push(macro_expand(e, env.clone())?);
            }
            Ok(Ast::Begin(loc, exp))
        }
        Ast::If(loc, cond, t, f) => {
            let econd = macro_expand(*cond, env.clone())?;
            let et = macro_expand(*t, env.clone())?;
            let ef = if let Some(f) = f {
                Some(Box::new(macro_expand(*f, env)?))
            } else {
                None
            };
            Ok(Ast::If(loc, Box::new(econd), Box::new(et), ef))
        }
        Ast::Define(loc, id, expr) => {
            Ok(Ast::Define(loc, id, Box::new(macro_expand(*expr, env)?)))
        }
        Ast::Set(loc, id, expr) => Ok(Ast::Set(loc, id, Box::new(macro_expand(*expr, env)?))),
        Ast::Let(loc, bindings, body) => {
            let mut exp_b = Vec::new();
            for (id, v) in bindings {
                exp_b.push((id, macro_expand(v, env.clone())?));
            }
            let mut exp_body = Vec::new();
            for b in body {
                exp_body.push(macro_expand(b, env.clone())?);
            }
            Ok(Ast::Let(loc, exp_b, exp_body))
        }
        Ast::Lambda(loc, params, body) => {
            let mut exp_body = Vec::new();
            for b in body {
                exp_body.push(macro_expand(b, env.clone())?);
            }
            Ok(Ast::Lambda(loc, params, exp_body))
        }
        Ast::DefMacro(loc, id, expr) => {
            Ok(Ast::DefMacro(loc, id, Box::new(macro_expand(*expr, env)?)))
        }
        Ast::Quasiquote(loc, expr) => Ok(Ast::Quasiquote(
            loc,
            Box::new(macro_expand_quasiquote(*expr, env)?),
        )),
        _ => Ok(ast),
    }
}

pub fn macro_expand_quasiquote(ast: Ast, env: Rc<RefCell<Env>>) -> Result<Ast> {
    match ast {
        Ast::Unquote(loc, expr) => Ok(Ast::Unquote(loc, Box::new(macro_expand(*expr, env)?))),
        Ast::UnquoteSplicing(loc, expr) => Ok(Ast::UnquoteSplicing(
            loc,
            Box::new(macro_expand(*expr, env)?),
        )),
        Ast::List(loc, list) => {
            let mut exp = Vec::new();
            for item in list {
                exp.push(macro_expand_quasiquote(item, env.clone())?);
            }
            Ok(Ast::List(loc, exp))
        }
        _ => Ok(ast),
    }
}

pub fn read_all(line: &str) -> Result<Vec<Ast>> {
    let mut lex = Lexer::new(line);
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

pub fn execute_asts(asts: Vec<Ast>, env: Rc<RefCell<Env>>) -> Result<Value> {
    let mut last_val = Value::Nil;
    for ast in asts {
        let expanded = macro_expand(ast, env.clone())?;
        let mut chunk = Chunk::new();
        let mut compiler = Compiler::new(&mut chunk);
        compiler.compile(expanded)?;
        let mut vm = VM::new();
        last_val = vm.run(Rc::new(chunk), env.clone())?;
    }
    Ok(last_val)
}
