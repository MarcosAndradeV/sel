use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::ast::*;
use crate::compiler::*;
use crate::diagnostics::*;
use crate::internal;
use crate::internal::load_core_lib;
use crate::internal::read_script;
use crate::internal::value_type_name;
use crate::lexer::Loc;
use crate::types::Record;
use crate::types::intern;
use crate::types::lookup;

type Result<T> = std::result::Result<T, SelError>;

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

    pub fn insert_checked(&mut self, id: u32, val: Value) -> Option<Value> {
        self.bindings.insert(id, val)
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
    #[allow(unused)]
    // Location of call, we need also a name
    pub loc: Loc,
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

    pub fn run(&mut self, loc: Loc, chunk: Rc<Chunk>, env: Rc<RefCell<Env>>) -> Result<Value> {
        let mut frames = vec![CallFrame {
            loc,
            chunk,
            ip: 0,
            env,
        }];
        match self.run_internal(&mut frames) {
            Err(e) => {
                if let Some(last) = frames.last() {
                    Err(SelError::Trace(format!(
                        "runtime error at {}:\n{e}",
                        last.loc
                    )))
                } else {
                    Err(e)
                }
            }
            ok => ok,
        }
    }

    fn run_internal(&mut self, frames: &mut Vec<CallFrame>) -> Result<Value> {
        loop {
            let frame_idx = frames.len() - 1;
            let frame = &mut frames[frame_idx];

            if frame.ip >= frame.chunk.code.len() {
                // End of root chunk
                if frames.len() == 1 {
                    return Ok(self.stack.pop().unwrap_or(Value::Nil));
                } else {
                    return Err(SelError::Internal(
                        "Unexpected end of function bytecode".into(),
                    ));
                }
            }

            let (loc, instruction) = frame.chunk.code[frame.ip].clone();
            frame.ip += 1;
            match instruction {
                OpCode::Import(id) => {
                    let fp = PathBuf::from(lookup(loc.file_id));
                    let (modname, fp) = if let Some(parent) = fp.parent()
                        && parent.is_dir()
                    {
                        let sym = lookup(id);
                        let pth = parent.join(format!("{}.scm", sym));
                        (sym, pth)
                    } else {
                        todo!()
                    };
                    let src = read_script(&fp).map_err(|e| SelError::Internal(e.to_string()))?;
                    let asts = parse_all(&src, intern(&fp.to_string_lossy().to_string()))?;
                    let m_env = Rc::new(RefCell::new(Env::default()));
                    m_env.borrow_mut().parent = Some(load_core_lib());
                    let rec = import_module(&modname, asts, m_env)?;
                    let mut frame_env = frame.env.borrow_mut();
                    for (sym, val) in rec.into_fields() {
                        if frame_env.insert_checked(sym, val).is_some() {
                            todo!("bind clash. How we should this?")
                        }
                    }
                }
                OpCode::MakeRecord => {
                    let v = Value::Record(Record::new());
                    self.stack.push(v)
                }
                OpCode::AssocRecord(sym) => {
                    let value = self.stack.pop().unwrap();
                    if let Value::Record(mut rec) = self.stack.pop().unwrap() {
                        rec.fields_mut().insert(sym, value);
                        self.stack.push(Value::Record(rec))
                    } else {
                        unreachable!();
                    }
                }
                OpCode::GetRecord(field) => {
                    if let Value::Record(mut rec) = self.stack.pop().unwrap() {
                        if let Some(value) = rec.fields_mut().get(&field).cloned() {
                            self.stack.push(value);
                        } else {
                            self.stack.push(Value::Nil);
                        }
                    } else {
                        return Err(SelError::Runtime(
                            loc,
                            "(rget <record> <symbol>) requires record".into(),
                        ));
                    }
                }
                OpCode::SetRecord(field) => {
                    let value = self.stack.pop().unwrap();
                    if let Value::Record(mut rec) = self.stack.pop().unwrap() {
                        let Some(field) = rec.fields_mut().get_mut(&field) else {
                            return Err(SelError::Runtime(
                                loc,
                                format!("record does not have field {}", lookup(field)),
                            ));
                        };
                        *field = value;
                        self.stack.push(Value::Record(rec));
                    } else {
                        return Err(SelError::Runtime(
                            loc,
                            "(rget <record> <symbol>) requires record".into(),
                        ));
                    }
                }
                OpCode::Eq(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::is_equal(loc, args)?)
                }
                OpCode::Mod => {
                    let b = match self.stack.pop().unwrap() {
                        Value::Integer(i) => i,
                        _ => {
                            return Err(SelError::Runtime(loc, "modulo requires integer".into()));
                        }
                    };
                    let a = match self.stack.pop().unwrap() {
                        Value::Integer(i) => i,
                        _ => {
                            return Err(SelError::Runtime(loc, "modulo requires integer".into()));
                        }
                    };

                    self.stack.push(Value::Integer(a % b))
                }
                OpCode::Div(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::div(loc, args)?)
                }
                OpCode::Mul(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::mul(loc, args)?)
                }
                OpCode::Sum(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::sum(loc, args)?)
                }
                OpCode::Sub(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::sub(loc, args)?)
                }
                OpCode::NumEq(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::num_eq(loc, args)?)
                }
                OpCode::NumNotEq(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::num_noteq(loc, args)?)
                }
                OpCode::NumLt(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::num_lt(loc, args)?)
                }
                OpCode::NumGt(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::num_gt(loc, args)?)
                }
                OpCode::NumLte(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::num_lte(loc, args)?)
                }
                OpCode::NumGte(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::num_gte(loc, args)?)
                }
                OpCode::Cons => {
                    let tail = self.stack.pop().unwrap();
                    let head = self.stack.pop().unwrap();
                    match tail {
                        Value::List(l) => {
                            let mut new_l = vec![head];
                            new_l.extend(l);
                            self.stack.push(Value::List(new_l))
                        }
                        Value::Nil => self.stack.push(Value::List(vec![head])),
                        _ => self.stack.push(Value::List(vec![head, tail])),
                    }
                }
                OpCode::Car => match self.stack.pop().unwrap() {
                    Value::List(mut l) => {
                        if l.is_empty() {
                            return Err(SelError::Runtime(loc, "car on empty list".into()));
                        }
                        self.stack.push(l.remove(0))
                    }
                    _ => return Err(SelError::Runtime(loc, "car requires a list".into())),
                },
                OpCode::Cdr => match self.stack.pop().unwrap() {
                    Value::List(mut l) => {
                        if l.is_empty() {
                            return Err(SelError::Runtime(loc, "cdr on empty list".into()));
                        }
                        l.remove(0);
                        self.stack.push(if l.is_empty() {
                            Value::Nil
                        } else {
                            Value::List(l)
                        })
                    }
                    _ => return Err(SelError::Runtime(loc, "cdr requires a list".into())),
                },
                OpCode::Nth => {
                    let index = self.stack.pop().unwrap();
                    match self.stack.pop().unwrap() {
                        Value::List(mut l) => match index {
                            Value::Integer(index) => {
                                self.stack.push(if (index as usize) < l.len() {
                                    l.remove(index as usize)
                                } else {
                                    Value::Nil
                                })
                            }
                            _ => {
                                return Err(SelError::Runtime(
                                    loc,
                                    "nth requires a interger".into(),
                                ));
                            }
                        },
                        _ => return Err(SelError::Runtime(loc, "nth requires a list".into())),
                    }
                }
                OpCode::Count => match self.stack.pop().unwrap() {
                    Value::List(l) => self.stack.push(Value::Integer(l.len() as _)),
                    Value::String(s) => self.stack.push(Value::Integer(s.len() as _)),
                    Value::Nil => self.stack.push(Value::Integer(0)),
                    _ => return Err(SelError::Runtime(loc, "count requires a list".into())),
                },
                OpCode::Empty => match self.stack.pop().unwrap() {
                    Value::List(l) => self.stack.push(Value::Boolean(l.is_empty())),
                    Value::Nil => self.stack.push(Value::Boolean(true)),
                    Value::String(s) => self.stack.push(Value::Boolean(s.is_empty())),
                    _ => return Err(SelError::Runtime(loc, "empty requires a list".into())),
                },
                OpCode::IsNil => match self.stack.pop().unwrap() {
                    Value::Nil => self.stack.push(Value::Boolean(true)),
                    _ => self.stack.push(Value::Boolean(false)),
                },
                OpCode::IsList => match self.stack.pop().unwrap() {
                    Value::List(l) if !l.is_empty() => self.stack.push(Value::Boolean(true)),
                    _ => self.stack.push(Value::Boolean(false)),
                },
                OpCode::IsNumber => {
                    let value = self.stack.pop().unwrap();
                    self.stack.push(Value::Boolean(matches!(
                        value,
                        Value::Integer(_) | Value::Float(_)
                    )))
                }
                OpCode::IsString => {
                    let value = self.stack.pop().unwrap();
                    self.stack
                        .push(Value::Boolean(matches!(value, Value::String(_))))
                }
                OpCode::IsSymbol => {
                    let value = self.stack.pop().unwrap();
                    self.stack
                        .push(Value::Boolean(matches!(value, Value::Symbol(_))))
                }
                OpCode::IsFunction => {
                    let value = self.stack.pop().unwrap();
                    self.stack.push(Value::Boolean(matches!(
                        value,
                        Value::NativeFunction(_) | Value::Closure { .. }
                    )))
                }
                OpCode::TypeOf => {
                    let v = self.stack.pop().unwrap();
                    self.stack.push(Value::Symbol(intern(value_type_name(&v))))
                }
                OpCode::Not(arity) => {
                    let start = self.stack.len() - arity as usize;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::not(loc, args)?)
                }
                OpCode::Constant(idx) => {
                    self.stack.push(frame.chunk.constants[idx].clone());
                }
                OpCode::LoadVar(id) => {
                    if let Some(val) = frame.env.borrow().get(id) {
                        self.stack.push(val);
                    } else {
                        return Err(SelError::UndefinedVariable(loc, id));
                    }
                }
                OpCode::StoreVar(id) => {
                    let val = self.stack.last().unwrap().clone();
                    if !frame.env.borrow_mut().set(id, val) {
                        return Err(SelError::UnboundVariable(loc, id));
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
                                    let rest_args =
                                        self.stack.split_off(self.stack.len() - (arg_count - i));
                                    let name = &lookup(*id)[1..];
                                    call_env.insert(intern(name), Value::List(rest_args));
                                    has_rest = true;
                                    break;
                                } else {
                                    let arg_idx = self.stack.len() - arg_count + i;
                                    call_env.insert(*id, self.stack[arg_idx].clone());
                                }
                            }
                            if !has_rest {
                                if params.len() != arg_count {
                                    return Err(SelError::ArityMismatch {
                                        loc,
                                        expected: params.len(),
                                        actual: arg_count,
                                    });
                                }
                                self.stack.truncate(self.stack.len() - arg_count);
                            }
                            self.stack.pop(); // pop callee
                            if is_tail_call {
                                frame.chunk = chunk;
                                frame.ip = 0;
                                frame.env = Rc::new(RefCell::new(call_env));
                            } else {
                                frames.push(CallFrame {
                                    loc,
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
                            self.stack.pop();
                            self.stack.push(f(loc, args)?);
                        }
                        Value::Macro { .. } => {
                            return Err(SelError::Runtime(
                                loc,
                                "Cannot call macro at runtime".into(),
                            ));
                        }
                        _ => {
                            return Err(SelError::Runtime(
                                loc,
                                format!("Attempt to call non-function value: {}", callee),
                            ));
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
                    if let Value::Closure { params, chunk, .. } = frame.chunk.constants[idx].clone()
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
                    if let Value::Macro { params, chunk, .. } = frame.chunk.constants[idx].clone() {
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
                                return Err(SelError::TypeError(
                                    loc,
                                    "unquote-splicing requires a list".into(),
                                ));
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
                    let expected = args_ast.len();

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
                            return Err(SelError::ArityMismatch {
                                loc,
                                expected,
                                actual: args.len(),
                            });
                        }
                    }

                    let mut vm = VM::new();
                    let result_val = vm.run(loc, chunk, Rc::new(RefCell::new(call_env)))?;

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
        Ast::Define(loc, id, expr) => Ok(Ast::Define(loc, id, Box::new(macro_expand(*expr, env)?))),
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

pub fn execute_asts(asts: Vec<Ast>, env: Rc<RefCell<Env>>) -> Result<Value> {
    let mut last_val = Value::Nil;
    for ast in asts {
        let loc = ast.loc();
        let expanded = macro_expand(ast, env.clone())?;
        let mut chunk = Chunk::new();
        let mut compiler = Compiler::new(&mut chunk);
        compiler.compile(expanded)?;
        let mut vm = VM::new();
        last_val = vm.run(loc, Rc::new(chunk), env.clone())?;
    }
    Ok(last_val)
}

pub fn import_module(module_name: &str, asts: Vec<Ast>, env: Rc<RefCell<Env>>) -> Result<Record<Value>> {
    let mut file_record = Record::new();
    for ast in asts {
        let loc = ast.loc();
        let expanded = macro_expand(ast, env.clone())?;
        let mut chunk = Chunk::new();
        let mut compiler = Compiler::new(&mut chunk);
        compiler.compile(expanded)?;
        let mut vm = VM::new();
        vm.run(loc, Rc::new(chunk), env.clone())?;
    }
    for (sym, value) in env.borrow().bindings.iter() {
        file_record.fields_mut().insert(
            intern(&format!("{module_name}/{}", lookup(*sym))),
            value.clone(),
        );
    }
    Ok(file_record)
}
