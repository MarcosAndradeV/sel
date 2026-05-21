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
use crate::parser::parse_all;
use crate::types::Record;
use crate::types::intern;
use crate::types::lookup;
use crate::value::Closure;
use crate::value::Macro;
use crate::value::*;

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

#[derive(Debug, Clone)]
pub struct CallFrame {
    #[allow(unused)]
    // Location of call, we need also a name
    pub loc: Loc,
    pub chunk: Rc<Chunk>,
    pub ip: usize,
    pub env: Rc<RefCell<Env>>,
}

#[derive(Clone)]
pub struct CatchHandler {
    pub catch_ip: usize,
    pub frame_index: usize,
    pub stack_height: usize,
    pub env: Rc<RefCell<Env>>,
}

pub struct VM {
    pub stack: Vec<Value>,
    pub catch_handlers: Vec<CatchHandler>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            catch_handlers: Vec::new(),
        }
    }

    pub fn run(&mut self, loc: Loc, chunk: Rc<Chunk>, env: Rc<RefCell<Env>>) -> Result<Value> {
        let mut frames = vec![CallFrame {
            loc,
            chunk,
            ip: 0,
            env,
        }];
        loop {
            match self.run_internal(&mut frames) {
                Err(e) => {
                    if self.catch_handlers.is_empty() {
                        if let Some(last) = frames.last() {
                            return Err(SelError::Trace(format!(
                                "runtime error at {}:\n{e}",
                                last.loc
                            )));
                        } else {
                            return Err(e);
                        }
                    } else {
                        self.handle_error(&mut frames, e)?;
                    }
                }
                ok => return ok,
            }
        }
    }

    fn handle_error(&mut self, frames: &mut Vec<CallFrame>, err: SelError) -> Result<()> {
        if let Some(handler) = self.catch_handlers.pop() {
            // Unwind frames to the saved frame_index
            frames.truncate(handler.frame_index + 1);

            // Restore the environment and IP of that target frame
            let target_frame = &mut frames[handler.frame_index];
            target_frame.env = handler.env;
            target_frame.ip = handler.catch_ip;

            // Unwind operand stack to the saved stack_height
            self.stack.truncate(handler.stack_height);

            // Push the error message as a String
            let err_msg = err.to_string();
            self.stack.push(Value::String(Rc::new(err_msg)));

            // Clean up any other catch handlers that were registered inside frames we just unwound
            let frame_count = frames.len();
            self.catch_handlers.retain(|h| h.frame_index < frame_count);

            Ok(())
        } else {
            Err(err)
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
                OpCode::Import(id, alias) => {
                    let fp = PathBuf::from(lookup(loc.file_id));
                    let sym = lookup(id);
                    
                    let (modname, fp) = if lookup(loc.file_id) != "<repl>"
                        && fp.parent().map_or(false, |p| p.is_dir() && p != std::path::Path::new(""))
                    {
                        let parent = fp.parent().unwrap();
                        let pth = parent.join(format!("{}.scm", sym));
                        (sym.clone(), pth)
                    } else {
                        let current = std::env::current_dir().unwrap_or_default();
                        let pth = current.join(format!("{}.scm", sym));
                        (sym.clone(), pth)
                    };
                    
                    let src = read_script(&fp).map_err(|e| SelError::Internal(e.to_string()))?;
                    let asts = parse_all(&src, intern(fp.to_string_lossy().as_ref()))?;
                    let m_env = Rc::new(RefCell::new(Env::default()));
                    m_env.borrow_mut().parent = Some(load_core_lib());
                    
                    // Extract base module name (e.g. "tests/math" -> "math")
                    let base_name = PathBuf::from(&modname)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&modname)
                        .to_string();
                        
                    // Determine namespace prefix
                    let prefix = if let Some(alias_id) = alias {
                        lookup(alias_id)
                    } else {
                        base_name
                    };
                    
                    let rec = import_module(&prefix, asts, m_env)?;
                    let mut frame_env = frame.env.borrow_mut();
                    for (sym, val) in rec.into_fields() {
                        frame_env.insert(sym, val);
                    }
                }
                OpCode::MakeRecord => {
                    let v = Value::Record(Rc::new(Record::new()));
                    self.stack.push(v)
                }
                OpCode::AssocRecord(sym) => {
                    let value = self.stack.pop().unwrap();
                    if let Value::Record(mut rec) = self.stack.pop().unwrap() {
                        Rc::make_mut(&mut rec).fields_mut().insert(sym, value);
                        self.stack.push(Value::Record(rec))
                    } else {
                        unreachable!();
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
                            new_l.extend(l.iter().cloned());
                            self.stack.push(Value::List(Rc::new(new_l)))
                        }
                        Value::Nil => self.stack.push(Value::List(Rc::new(vec![head]))),
                        _ => self.stack.push(Value::List(Rc::new(vec![head, tail]))),
                    }
                }
                OpCode::Car => match self.stack.pop().unwrap() {
                    Value::List(l) => {
                        if l.is_empty() {
                            return Err(SelError::Runtime(loc, "car on empty list".into()));
                        }
                        self.stack.push(l[0].clone())
                    }
                    _ => return Err(SelError::Runtime(loc, "car requires a list".into())),
                },
                OpCode::Cdr => match self.stack.pop().unwrap() {
                    Value::List(l) => {
                        if l.is_empty() {
                            return Err(SelError::Runtime(loc, "cdr on empty list".into()));
                        }
                        if l.len() == 1 {
                            self.stack.push(Value::Nil)
                        } else {
                            let mut new_l = Vec::with_capacity(l.len() - 1);
                            new_l.extend_from_slice(&l[1..]);
                            self.stack.push(Value::List(Rc::new(new_l)))
                        }
                    }
                    _ => return Err(SelError::Runtime(loc, "cdr requires a list".into())),
                },
                OpCode::Nth => {
                    let index = self.stack.pop().unwrap();
                    match self.stack.pop().unwrap() {
                        Value::List(l) => match index {
                            Value::Integer(index) => {
                                self.stack.push(if (index as usize) < l.len() {
                                    l[index as usize].clone()
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
                    let callee = self.stack[self.stack.len() - arg_count - 1].clone();
                    match callee {
                        Value::Closure(c) => {
                            let params = c.params.clone();
                            let chunk = c.chunk.clone();
                            let c_env = c.env.clone();
                            let mut call_env = Env::new(Some(c_env));
                            let mut has_rest = false;
                            for (i, id) in params.iter().enumerate() {
                                if lookup(*id).starts_with('&') {
                                    let rest_args =
                                        self.stack.split_off(self.stack.len() - (arg_count - i));
                                    let name = &lookup(*id)[1..];
                                    call_env.insert(intern(name), Value::List(Rc::new(rest_args)));
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
                            frames.push(CallFrame {
                                loc,
                                chunk,
                                ip: 0,
                                env: Rc::new(RefCell::new(call_env)),
                            });
                        }
                        Value::NativeFunction(f) => {
                            let mut args = Vec::with_capacity(arg_count);
                            let start = self.stack.len() - arg_count;
                            args.extend(self.stack.drain(start..));
                            self.stack.pop(); // pop callee
                            self.stack.push(f(loc, args)?);
                        }
                        Value::Macro(_) => {
                            return Err(SelError::Runtime(
                                loc,
                                "Cannot call macro at runtime".into(),
                            ));
                        }
                        Value::NativeClosure(f) => {
                            let mut args = Vec::with_capacity(arg_count);
                            let start = self.stack.len() - arg_count;
                            args.extend(self.stack.drain(start..));
                            self.stack.pop(); // pop callee
                            self.stack.push((f.0)(loc, args)?);
                        }
                        Value::Record(_) => match arg_count {
                            1 => {
                                let s = self.stack.pop().unwrap();
                                if !matches!(s, Value::Symbol(_)) {
                                    return Err(SelError::Runtime(
                                        loc,
                                        format!("Attempt to call non-function value: {}", callee),
                                    ));
                                }
                                let r = self.stack.pop().unwrap();
                                let value = internal::rget(loc, vec![r, s])?;
                                self.stack.push(value);
                            }
                            2 => {
                                let v = self.stack.pop().unwrap();
                                let s = self.stack.pop().unwrap();
                                if !matches!(s, Value::Symbol(_)) {
                                    return Err(SelError::Runtime(
                                        loc,
                                        format!("Attempt to call non-function value: {}", callee),
                                    ));
                                }
                                let r = self.stack.pop().unwrap();
                                let value = internal::rset(loc, vec![r, s, v])?;
                                self.stack.push(value);
                            }
                            _ => {
                                return Err(SelError::Runtime(
                                    loc,
                                    format!("Attempt to call non-function value: {}", callee),
                                ));
                            }
                        },
                        Value::Symbol(sym) => match arg_count {
                            1 => {
                                let r = self.stack.pop().unwrap();
                                if let Value::Record(_) = r {
                                    let value = internal::rget(loc, vec![r, Value::Symbol(sym)])?;
                                    self.stack.pop(); // pop callee (the symbol)
                                    self.stack.push(value);
                                } else {
                                    return Err(SelError::Runtime(
                                        loc,
                                        format!("Attempt to call symbol on non-record: {}", r),
                                    ));
                                }
                            }
                            2 => {
                                let v = self.stack.pop().unwrap();
                                let r = self.stack.pop().unwrap();
                                if let Value::Record(_) = r {
                                    let value = internal::rset(loc, vec![r, Value::Symbol(sym), v])?;
                                    self.stack.pop(); // pop callee (the symbol)
                                    self.stack.push(value);
                                } else {
                                    return Err(SelError::Runtime(
                                        loc,
                                        format!("Attempt to call symbol on non-record: {}", r),
                                    ));
                                }
                            }
                            _ => {
                                return Err(SelError::Runtime(
                                    loc,
                                    format!("Attempt to call non-function value: {}", callee),
                                ));
                            }
                        },
                        _ => {
                            return Err(SelError::Runtime(
                                loc,
                                format!("Attempt to call non-function value: {}", callee),
                            ));
                        }
                    }
                }
                OpCode::TailCall(arg_count) => {
                    let callee = self.stack[self.stack.len() - arg_count - 1].clone();
                    match callee {
                        Value::Closure(c) => {
                            let params = c.params.clone();
                            let chunk = c.chunk.clone();
                            let c_env = c.env.clone();
                            let mut call_env = Env::new(Some(c_env));
                            let mut has_rest = false;
                            for (i, id) in params.iter().enumerate() {
                                if lookup(*id).starts_with('&') {
                                    let rest_args =
                                        self.stack.split_off(self.stack.len() - (arg_count - i));
                                    let name = &lookup(*id)[1..];
                                    call_env.insert(intern(name), Value::List(Rc::new(rest_args)));
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
                            frame.chunk = chunk;
                            frame.ip = 0;
                            frame.env = Rc::new(RefCell::new(call_env));
                        }
                        Value::NativeFunction(f) => {
                            let mut args = Vec::with_capacity(arg_count);
                            let start = self.stack.len() - arg_count;
                            args.extend(self.stack.drain(start..));
                            self.stack.pop(); // pop callee
                            let res = f(loc, args)?;
                            
                            frames.pop();
                            self.stack.push(res);
                            if frames.is_empty() {
                                return Ok(self.stack.pop().unwrap());
                            }
                        }
                        Value::Macro(_) => {
                            return Err(SelError::Runtime(
                                loc,
                                "Cannot call macro at runtime".into(),
                            ));
                        }
                        Value::NativeClosure(f) => {
                            let mut args = Vec::with_capacity(arg_count);
                            let start = self.stack.len() - arg_count;
                            args.extend(self.stack.drain(start..));
                            self.stack.pop(); // pop callee
                            let res = (f.0)(loc, args)?;
                            
                            frames.pop();
                            self.stack.push(res);
                            if frames.is_empty() {
                                return Ok(self.stack.pop().unwrap());
                            }
                        }
                        Value::Record(_) => {
                            let res = match arg_count {
                                1 => {
                                    let s = self.stack.pop().unwrap();
                                    if !matches!(s, Value::Symbol(_)) {
                                        return Err(SelError::Runtime(
                                            loc,
                                            format!("Attempt to call non-function value: {}", callee),
                                        ));
                                    }
                                    let r = self.stack.pop().unwrap();
                                    internal::rget(loc, vec![r, s])?
                                }
                                2 => {
                                    let v = self.stack.pop().unwrap();
                                    let s = self.stack.pop().unwrap();
                                    if !matches!(s, Value::Symbol(_)) {
                                        return Err(SelError::Runtime(
                                            loc,
                                            format!("Attempt to call non-function value: {}", callee),
                                        ));
                                    }
                                    let r = self.stack.pop().unwrap();
                                    internal::rset(loc, vec![r, s, v])?
                                }
                                _ => {
                                    return Err(SelError::Runtime(
                                        loc,
                                        format!("Attempt to call non-function value: {}", callee),
                                    ));
                                }
                            };
                            self.stack.pop(); // pop callee
                            frames.pop();
                            self.stack.push(res);
                            if frames.is_empty() {
                                return Ok(self.stack.pop().unwrap());
                            }
                        }
                        Value::Symbol(sym) => {
                            let res = match arg_count {
                                1 => {
                                    let r = self.stack.pop().unwrap();
                                    if let Value::Record(_) = r {
                                        internal::rget(loc, vec![r, Value::Symbol(sym)])?
                                    } else {
                                        return Err(SelError::Runtime(
                                            loc,
                                            format!("Attempt to call symbol on non-record: {}", r),
                                        ));
                                    }
                                }
                                2 => {
                                    let v = self.stack.pop().unwrap();
                                    let r = self.stack.pop().unwrap();
                                    if let Value::Record(_) = r {
                                        internal::rset(loc, vec![r, Value::Symbol(sym), v])?
                                    } else {
                                        return Err(SelError::Runtime(
                                            loc,
                                            format!("Attempt to call symbol on non-record: {}", r),
                                        ));
                                    }
                                }
                                _ => {
                                    return Err(SelError::Runtime(
                                        loc,
                                        format!("Attempt to call non-function value: {}", callee),
                                    ));
                                }
                            };
                            self.stack.pop(); // pop callee (the symbol)
                            frames.pop();
                            self.stack.push(res);
                            if frames.is_empty() {
                                return Ok(self.stack.pop().unwrap());
                            }
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
                    let frame_idx = frames.len() - 1;
                    self.catch_handlers.retain(|h| h.frame_index < frame_idx);
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
                OpCode::RegisterCatch(catch_ip) => {
                    self.catch_handlers.push(CatchHandler {
                        catch_ip,
                        frame_index: frame_idx,
                        stack_height: self.stack.len(),
                        env: frame.env.clone(),
                    });
                }
                OpCode::UnregisterCatch => {
                    self.catch_handlers.pop();
                }
                OpCode::Yield => {
                    let yielded_val = self.stack.pop().unwrap_or(Value::Nil);
                    return Ok(yielded_val);
                }
                OpCode::CoResume => {
                    let arg = self.stack.pop().unwrap_or(Value::Nil);
                    let coroutine_val = self.stack.pop().ok_or_else(|| {
                        SelError::Runtime(frame.loc, "co-resume: missing coroutine on stack".into())
                    })?;

                    if let Value::Coroutine(co) = coroutine_val {
                        let state = co.state.get();
                        if state == CoroutineState::Dead {
                            return Err(SelError::Runtime(frame.loc, "Cannot resume a dead coroutine".into()));
                        }
                        if state == CoroutineState::Running {
                            return Err(SelError::Runtime(frame.loc, "Cannot resume a running coroutine (re-entry is forbidden)".into()));
                        }

                        co.state.set(CoroutineState::Running);

                        let co_stack = co.operand_stack.take();
                        let old_stack = std::mem::replace(&mut self.stack, co_stack);
                        let mut co_frames = co.frames.take();

                        if co_frames.is_empty() {
                            let mut call_env = Env::new(Some(co.closure.env.clone()));
                            let params = &co.closure.params;
                            if !params.is_empty() {
                                let first_param = params[0];
                                if crate::types::lookup(first_param).starts_with('&') {
                                    let name = &crate::types::lookup(first_param)[1..];
                                    call_env.insert(crate::types::intern(name), Value::List(Rc::new(vec![arg.clone()])));
                                } else {
                                    call_env.insert(first_param, arg.clone());
                                }
                            }
                            co_frames.push(CallFrame {
                                loc: frame.loc,
                                chunk: co.closure.chunk.clone(),
                                ip: 0,
                                env: Rc::new(RefCell::new(call_env)),
                            });
                        } else {
                            self.stack.push(arg);
                        }

                        let res = self.run_internal(&mut co_frames);

                        match res {
                            Ok(val) => {
                                if co_frames.is_empty() {
                                    co.state.set(CoroutineState::Dead);
                                } else {
                                    co.state.set(CoroutineState::Suspended);
                                }
                                *co.frames.borrow_mut() = co_frames;
                                let final_co_stack = std::mem::replace(&mut self.stack, old_stack);
                                *co.operand_stack.borrow_mut() = final_co_stack;
                                self.stack.push(val);
                            }
                            Err(e) => {
                                co.state.set(CoroutineState::Dead);
                                *co.frames.borrow_mut() = co_frames;
                                let final_co_stack = std::mem::replace(&mut self.stack, old_stack);
                                *co.operand_stack.borrow_mut() = final_co_stack;
                                return Err(e);
                            }
                        }
                    } else {
                        return Err(SelError::Runtime(
                            frame.loc,
                            format!("co-resume: expected coroutine but got {}", coroutine_val),
                        ));
                    }
                }
                OpCode::MakeClosure(idx) => {
                    if let Value::Closure(c) = frame.chunk.constants[idx].clone() {
                        let closure = Value::Closure(Rc::new(Closure {
                            params: c.params.clone(),
                            chunk: c.chunk.clone(),
                            env: frame.env.clone(),
                        }));
                        self.stack.push(closure);
                    }
                }
                OpCode::MakeMacro(id, idx) => {
                    if let Value::Macro(m) = frame.chunk.constants[idx].clone() {
                        let mac = Value::Macro(Rc::new(Macro {
                            params: m.params.clone(),
                            chunk: m.chunk.clone(),
                            env: frame.env.clone(),
                        }));
                        frame.env.borrow_mut().insert(id, mac.clone());
                        self.stack.push(Value::Symbol(id));
                    }
                }
                OpCode::MakeList(count) => {
                    let mut items = Vec::with_capacity(count);
                    let start = self.stack.len() - count;
                    items.extend(self.stack.drain(start..));
                    self.stack.push(Value::List(Rc::new(items)));
                }
                OpCode::ConcatList(count) => {
                    let mut items = Vec::new();
                    let start = self.stack.len() - count;
                    for val in self.stack.drain(start..) {
                        match val {
                            Value::List(l) => items.extend(l.iter().cloned()),
                            Value::Nil => {}
                            _ => {
                                return Err(SelError::TypeError(
                                    loc,
                                    "unquote-splicing requires a list".into(),
                                ));
                            }
                        }
                    }
                    self.stack.push(Value::List(Rc::new(items)));
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
                if let Some(Value::Macro(mac)) = macro_opt {
                    let params = mac.params.clone();
                    let chunk = mac.chunk.clone();
                    let m_env = mac.env.clone();

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
                            call_env.insert(intern(name), Value::List(Rc::new(rest_args)));
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
        Ast::Record(loc, record) => {
            let mut exp_fields = Vec::new();
            for (sym, arg) in record {
                exp_fields.push((sym, macro_expand(arg, env.clone())?));
            }
            Ok(Ast::Record(loc, exp_fields))
        }
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
        Ast::Record(loc, record) => {
            let mut exp_fields = Vec::new();
            for (sym, arg) in record {
                exp_fields.push((sym, macro_expand_quasiquote(arg, env.clone())?));
            }
            Ok(Ast::Record(loc, exp_fields))
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

pub fn import_module(
    module_name: &str,
    asts: Vec<Ast>,
    env: Rc<RefCell<Env>>,
) -> Result<Record<Value>> {
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
