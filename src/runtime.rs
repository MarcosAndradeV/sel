use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::ast::*;
use crate::compiler::*;
use crate::diagnostics::*;
use crate::internal;
use crate::internal::load_core_lib;
use crate::internal::read_script;
use crate::lexer::Loc;
use crate::parser::parse_all;
use crate::types::Record;
use crate::types::intern;
use crate::types::lookup;
use crate::value::Closure;
use crate::value::Macro;
use crate::value::*;

type Result<T> = std::result::Result<T, SelError>;

#[derive(Debug)]
pub struct Env {
    pub bindings: FxHashMap<u32, Value>,
    pub parent: Option<Rc<RefCell<Env>>>,
    pub private_bindings: FxHashSet<u32>,
    pub current_visibility_public: bool,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            bindings: FxHashMap::default(),
            parent: None,
            private_bindings: FxHashSet::default(),
            current_visibility_public: true,
        }
    }
}

impl Env {
    fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Self {
            bindings: FxHashMap::default(),
            parent,
            private_bindings: FxHashSet::default(),
            current_visibility_public: true,
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
        if !self.current_visibility_public {
            self.private_bindings.insert(id);
        } else {
            self.private_bindings.remove(&id);
        }
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
    pub locals: Vec<Value>,
}

#[derive(Clone)]
pub struct CatchHandler {
    pub catch_ip: usize,
    pub frame_index: usize,
    pub stack_height: usize,
    pub env: Rc<RefCell<Env>>,
}

#[derive(Default)]
pub struct VM {
    pub stack: Vec<Value>,
    pub catch_handlers: Vec<CatchHandler>,
    pub sandbox_root: Option<PathBuf>,
    pub module_cache: FxHashMap<PathBuf, Vec<(u32, Value)>>,
}

fn check_sandbox(path: &std::path::Path, sandbox_root: &std::path::Path, loc: Loc) -> Result<()> {
    let canonical_path = path.canonicalize().map_err(|e| {
        SelError::SandboxViolation(
            loc,
            format!("Failed to resolve path {}: {}", path.display(), e),
        )
    })?;
    let canonical_root = sandbox_root.canonicalize().map_err(|e| {
        SelError::SandboxViolation(
            loc,
            format!(
                "Failed to resolve sandbox root {}: {}",
                sandbox_root.display(),
                e
            ),
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(SelError::SandboxViolation(
            loc,
            format!(
                "Sandbox violation: path {} is outside root {}",
                canonical_path.display(),
                canonical_root.display()
            ),
        ));
    }
    Ok(())
}

fn resolve_module_path(spec: &str, caller_file_id: u32, loc: Loc) -> Result<(String, PathBuf)> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let caller_file = lookup(caller_file_id);
    let caller_path = PathBuf::from(&caller_file);

    let mut add_candidates = |base: &std::path::Path, spec_str: &str| {
        let p = base.join(spec_str);
        if p.is_file() {
            candidates.push(p.clone());
        }
        let with_scm = if spec_str.ends_with(".scm") {
            base.join(spec_str)
        } else {
            base.join(format!("{spec_str}.scm"))
        };
        candidates.push(with_scm);

        let mod_scm = base.join(spec_str).join("mod.scm");
        candidates.push(mod_scm);
    };

    // 1. Direct path if spec exists as a file directly
    let direct_p = PathBuf::from(spec);
    if direct_p.is_file() {
        return Ok((spec.to_string(), direct_p));
    }

    // 2. Relative to caller file's parent directory
    if caller_file != "<repl>"
        && caller_file != "<embedded>"
        && let Some(parent) = caller_path.parent()
        && parent.is_dir()
        && parent != std::path::Path::new("")
    {
        add_candidates(parent, spec);
    }

    // 3. Relative to current working directory
    if let Ok(current) = std::env::current_dir() {
        add_candidates(&current, spec);
    }

    // 4. In SEL_PATH environment variable
    if let Ok(sel_path) = std::env::var("SEL_PATH") {
        for dir in std::env::split_paths(&sel_path) {
            add_candidates(&dir, spec);
        }
    }

    // Check candidate paths
    for cand in &candidates {
        if cand.is_file() {
            return Ok((spec.to_string(), cand.clone()));
        }
    }

    Err(SelError::Runtime(
        loc,
        format!(
            "Cannot find module `{}`. Searched candidate paths:\n{}",
            spec,
            candidates
                .iter()
                .map(|p| format!("  - {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    ))
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            catch_handlers: Vec::new(),
            sandbox_root: None,
            module_cache: FxHashMap::default(),
        }
    }

    pub fn run(
        &mut self,
        loc: Loc,
        chunk: Rc<Chunk>,
        env: Rc<RefCell<Env>>,
        locals: Vec<Value>,
    ) -> Result<Value> {
        let mut frames = vec![CallFrame {
            loc,
            chunk,
            ip: 0,
            env,
            locals,
        }];
        loop {
            match self.run_internal(&mut frames) {
                Err(e) => {
                    if self.catch_handlers.is_empty() {
                        return Err(e);
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
            self.stack.push(Value::make_string(&err_msg));

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

            let instr_start = frame.ip;
            let loc = frame.chunk.get_loc(instr_start);
            let instruction_tag = frame.chunk.code[frame.ip];
            frame.ip += 1;

            let read_u8 = |f: &mut CallFrame| {
                let val = f.chunk.code[f.ip];
                f.ip += 1;
                val
            };

            let read_u32 = |f: &mut CallFrame| {
                let val = u32::from_le_bytes(f.chunk.code[f.ip..f.ip + 4].try_into().unwrap());
                f.ip += 4;
                val
            };

            let read_usize = |f: &mut CallFrame| {
                let val = u32::from_le_bytes(f.chunk.code[f.ip..f.ip + 4].try_into().unwrap());
                f.ip += 4;
                val as usize
            };

            let read_bool = |f: &mut CallFrame| {
                let val = f.chunk.code[f.ip] != 0;
                f.ip += 1;
                val
            };

            match instruction_tag {
                1 => {
                    // Constant
                    let idx = read_usize(frame);
                    self.stack.push(frame.chunk.constants[idx].clone());
                }
                2 => {
                    // LoadVar
                    let id = read_u32(frame);
                    if let Some(val) = frame.env.borrow().get(id) {
                        self.stack.push(val);
                    } else {
                        return Err(SelError::UndefinedVariable(loc, id));
                    }
                }
                3 => {
                    // StoreVar
                    let id = read_u32(frame);
                    let val = self.stack.last().unwrap().clone();
                    if !frame.env.borrow_mut().set(id, val) {
                        return Err(SelError::UnboundVariable(loc, id));
                    }
                }
                4 => {
                    // DefVar
                    let id = read_u32(frame);
                    let val = self.stack.pop().unwrap();
                    frame.env.borrow_mut().insert(id, val);
                    self.stack.push(Value::Symbol(id));
                }
                5 => {
                    // LoadLocal
                    let idx = read_u8(frame);
                    let val = frame.locals[idx as usize].clone();
                    self.stack.push(val);
                }
                6 => {
                    // StoreLocal
                    let idx = read_u8(frame);
                    let val = self.stack.last().unwrap().clone();
                    frame.locals[idx as usize] = val;
                }
                7 => {
                    // Pop
                    self.stack.pop();
                }
                8 => {
                    // JumpIfFalse
                    let offset = read_usize(frame);
                    let val = self.stack.last().unwrap();
                    let is_false = matches!(val, Value::Boolean(false));
                    if is_false {
                        frame.ip = offset;
                    }
                }
                9 => {
                    // Jump
                    let offset = read_usize(frame);
                    frame.ip = offset;
                }
                10 => {
                    // Call
                    let arg_count = read_usize(frame);
                    let callee = self.stack[self.stack.len() - arg_count - 1].clone();
                    match callee {
                        Value::Closure(c) => {
                            let params = &c.params;
                            let chunk = c.chunk.clone();
                            let c_env = c.env.clone();
                            let mut call_env = Env::new(Some(c_env));
                            let mut locals = Vec::with_capacity(params.len());

                            if let Some((rest_idx, rest_id)) = c.rest_param {
                                if arg_count < rest_idx {
                                    return Err(SelError::ArityMismatch {
                                        loc,
                                        expected: rest_idx,
                                        actual: arg_count,
                                    });
                                }
                                let stack_start = self.stack.len() - arg_count;
                                for i in 0..rest_idx {
                                    let arg_val = self.stack[stack_start + i].clone();
                                    call_env.insert(params[i], arg_val.clone());
                                    locals.push(arg_val);
                                }
                                let rest_args = self.stack.split_off(stack_start + rest_idx);
                                let rest_val = Value::make_list(rest_args);
                                call_env.insert(rest_id, rest_val.clone());
                                locals.push(rest_val);
                                self.stack.pop(); // pop callee
                            } else {
                                if params.len() != arg_count {
                                    return Err(SelError::ArityMismatch {
                                        loc,
                                        expected: params.len(),
                                        actual: arg_count,
                                    });
                                }
                                let stack_start = self.stack.len() - arg_count;
                                for i in 0..arg_count {
                                    let arg_val = self.stack[stack_start + i].clone();
                                    call_env.insert(params[i], arg_val.clone());
                                    locals.push(arg_val);
                                }
                                self.stack.truncate(stack_start);
                                self.stack.pop(); // pop callee
                            }
                            frames.push(CallFrame {
                                loc,
                                chunk,
                                ip: 0,
                                env: Rc::new(RefCell::new(call_env)),
                                locals,
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
                                    let value =
                                        internal::rset(loc, vec![r, Value::Symbol(sym), v])?;
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
                11 => {
                    // TailCall
                    let arg_count = read_usize(frame);
                    let callee = self.stack[self.stack.len() - arg_count - 1].clone();
                    match callee {
                        Value::Closure(c) => {
                            let params = &c.params;
                            let chunk = c.chunk.clone();
                            let c_env = c.env.clone();
                            let mut call_env = Env::new(Some(c_env));
                            let mut locals = Vec::with_capacity(params.len());

                            if let Some((rest_idx, rest_id)) = c.rest_param {
                                if arg_count < rest_idx {
                                    return Err(SelError::ArityMismatch {
                                        loc,
                                        expected: rest_idx,
                                        actual: arg_count,
                                    });
                                }
                                let stack_start = self.stack.len() - arg_count;
                                for i in 0..rest_idx {
                                    let arg_val = self.stack[stack_start + i].clone();
                                    call_env.insert(params[i], arg_val.clone());
                                    locals.push(arg_val);
                                }
                                let rest_args = self.stack.split_off(stack_start + rest_idx);
                                let rest_val = Value::make_list(rest_args);
                                call_env.insert(rest_id, rest_val.clone());
                                locals.push(rest_val);
                                self.stack.pop(); // pop callee
                            } else {
                                if params.len() != arg_count {
                                    return Err(SelError::ArityMismatch {
                                        loc,
                                        expected: params.len(),
                                        actual: arg_count,
                                    });
                                }
                                let stack_start = self.stack.len() - arg_count;
                                for i in 0..arg_count {
                                    let arg_val = self.stack[stack_start + i].clone();
                                    call_env.insert(params[i], arg_val.clone());
                                    locals.push(arg_val);
                                }
                                self.stack.truncate(stack_start);
                                self.stack.pop(); // pop callee
                            }
                            frame.chunk = chunk;
                            frame.ip = 0;
                            frame.env = Rc::new(RefCell::new(call_env));
                            frame.locals = locals;
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
                                            format!(
                                                "Attempt to call non-function value: {}",
                                                callee
                                            ),
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
                                            format!(
                                                "Attempt to call non-function value: {}",
                                                callee
                                            ),
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
                12 => {
                    // MakeClosure
                    let idx = read_usize(frame);
                    if let Value::Closure(c) = &frame.chunk.constants[idx] {
                        let closure = Value::Closure(Rc::new(Closure {
                            params: c.params.clone(),
                            rest_param: c.rest_param,
                            chunk: c.chunk.clone(),
                            env: frame.env.clone(),
                        }));
                        self.stack.push(closure);
                    }
                }
                13 => {
                    // MakeMacro
                    let id = read_u32(frame);
                    let idx = read_usize(frame);
                    if let Value::Macro(m) = &frame.chunk.constants[idx] {
                        let mac = Value::Macro(Rc::new(Macro {
                            params: m.params.clone(),
                            rest_param: m.rest_param,
                            chunk: m.chunk.clone(),
                            env: frame.env.clone(),
                        }));
                        frame.env.borrow_mut().insert(id, mac.clone());
                        self.stack.push(Value::Symbol(id));
                    }
                }
                14 => {
                    // Return
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    let frame_idx = frames.len() - 1;
                    self.catch_handlers.retain(|h| h.frame_index < frame_idx);
                    frames.pop();
                    self.stack.push(result);
                    if frames.is_empty() {
                        return Ok(self.stack.pop().unwrap());
                    }
                }
                15 => {
                    // BuildEnv
                    let len = read_u32(frame) as usize;
                    let mut ids = Vec::with_capacity(len);
                    for _ in 0..len {
                        ids.push(read_u32(frame));
                    }
                    let mut let_env = Env::new(Some(frame.env.clone()));
                    let start = self.stack.len() - ids.len();
                    let vals: Vec<Value> = self.stack.drain(start..).collect();
                    for (id, val) in ids.into_iter().zip(vals) {
                        let_env.insert(id, val.clone());
                        frame.locals.push(val);
                    }
                    frame.env = Rc::new(RefCell::new(let_env));
                }
                16 => {
                    // PopEnv
                    let count = read_usize(frame);
                    let parent = frame.env.borrow().parent.clone().unwrap();
                    frame.env = parent;
                    let new_len = frame.locals.len().saturating_sub(count);
                    frame.locals.truncate(new_len);
                }
                17 => {
                    // RegisterCatch
                    let catch_ip = read_usize(frame);
                    self.catch_handlers.push(CatchHandler {
                        catch_ip,
                        frame_index: frame_idx,
                        stack_height: self.stack.len(),
                        env: frame.env.clone(),
                    });
                }
                18 => {
                    // UnregisterCatch
                    self.catch_handlers.pop();
                }
                19 => {
                    // Yield
                    let yielded_val = self.stack.pop().unwrap_or(Value::Nil);
                    return Ok(yielded_val);
                }
                20 => {
                    // CoResume
                    let arg = self.stack.pop().unwrap_or(Value::Nil);
                    let coroutine_val = self.stack.pop().ok_or_else(|| {
                        SelError::Runtime(frame.loc, "co-resume: missing coroutine on stack".into())
                    })?;

                    if let Value::Coroutine(co) = coroutine_val {
                        let state = co.state.get();
                        if state == CoroutineState::Dead {
                            return Err(SelError::Runtime(
                                frame.loc,
                                "Cannot resume a dead coroutine".into(),
                            ));
                        }
                        if state == CoroutineState::Running {
                            return Err(SelError::Runtime(
                                frame.loc,
                                "Cannot resume a running coroutine (re-entry is forbidden)".into(),
                            ));
                        }

                        co.state.set(CoroutineState::Running);

                        let co_stack = co.operand_stack.take();
                        let old_stack = std::mem::replace(&mut self.stack, co_stack);
                        let mut co_frames = co.frames.take();

                        if co_frames.is_empty() {
                            let mut call_env = Env::new(Some(co.closure.env.clone()));
                            let params = &co.closure.params;
                            let mut locals = Vec::new();
                            if !params.is_empty() {
                                let first_param = params[0];
                                if let Some((0, rest_id)) = co.closure.rest_param {
                                    let rest_val = Value::make_list(vec![arg.clone()]);
                                    call_env.insert(rest_id, rest_val.clone());
                                    locals.push(rest_val);
                                } else {
                                    call_env.insert(first_param, arg.clone());
                                    locals.push(arg.clone());
                                }
                            }
                            co_frames.push(CallFrame {
                                loc: frame.loc,
                                chunk: co.closure.chunk.clone(),
                                ip: 0,
                                env: Rc::new(RefCell::new(call_env)),
                                locals,
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
                21 => {
                    // Import
                    let id = read_u32(frame);
                    let has_alias = read_u8(frame) != 0;
                    let alias = if has_alias {
                        Some(read_u32(frame))
                    } else {
                        None
                    };
                    let sym = lookup(id);
                    let (modname, fp) = resolve_module_path(&sym, loc.file_id, loc)?;

                    if let Some(ref root) = self.sandbox_root {
                        check_sandbox(&fp, root, loc)?;
                    }

                    // Extract base module name (e.g. "tests/math" -> "math", "pkg/mod.scm" -> "pkg")
                    let base_name = if fp.file_name().and_then(|s| s.to_str()) == Some("mod.scm") {
                        fp.parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .unwrap_or(&modname)
                            .to_string()
                    } else {
                        PathBuf::from(&modname)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&modname)
                            .to_string()
                    };

                    // Determine namespace prefix
                    let prefix = if let Some(alias_id) = alias {
                        lookup(alias_id)
                    } else {
                        base_name
                    };

                    let exports = if let Some(exports) = self.module_cache.get(&fp) {
                        exports.clone()
                    } else {
                        let src = read_script(&fp).map_err(|e| SelError::Internal(e.to_string()))?;
                        let mut diags = Vec::new();
                        let file_id = intern(fp.to_string_lossy().as_ref());
                        let asts = parse_all(&src, file_id, &mut diags);
                        let m_env = Rc::new(RefCell::new(Env::default()));
                        m_env.borrow_mut().parent = Some(load_core_lib());

                        execute_asts_sandboxed(asts, m_env.clone(), self.sandbox_root.clone())?;
                        let mut exports = Vec::new();
                        for (sym, val) in m_env.borrow().bindings.iter() {
                            if m_env.borrow().private_bindings.contains(sym) {
                                continue;
                            }
                            exports.push((*sym, val.clone()));
                        }
                        self.module_cache.insert(fp.clone(), exports.clone());
                        exports
                    };

                    let mut frame_env = frame.env.borrow_mut();
                    for (sym, val) in exports {
                        let prefixed = intern(&format!("{prefix}/{}", lookup(sym)));
                        frame_env.insert(prefixed, val);
                    }
                }
                22 => {
                    // SetVisibility
                    let is_public = read_bool(frame);
                    frame.env.borrow_mut().current_visibility_public = is_public;
                    self.stack.push(Value::Nil);
                }
                23 => {
                    // MakeRecord
                    let v = Value::Record(Rc::new(Record::new()));
                    self.stack.push(v);
                }
                24 => {
                    // AssocRecord
                    let sym = read_u32(frame);
                    let value = self.stack.pop().unwrap();
                    if let Value::Record(mut rec) = self.stack.pop().unwrap() {
                        Rc::make_mut(&mut rec).fields_mut().insert(sym, value);
                        self.stack.push(Value::Record(rec));
                    } else {
                        unreachable!();
                    }
                }
                25 => {
                    // MakeList
                    let count = read_usize(frame);
                    let start = self.stack.len() - count;
                    let args = self.stack.drain(start..).collect();
                    self.stack.push(Value::make_list(args));
                }
                26 => {
                    // ConcatList
                    let count = read_usize(frame);
                    let mut items = imbl::Vector::new();
                    let start = self.stack.len() - count;
                    for val in self.stack.drain(start..) {
                        match val {
                            Value::List(l) => items.append(*l),
                            Value::String(s) => items.extend(s.as_str().chars().map(Value::Char)),
                            Value::Nil => {}
                            _ => {
                                return Err(SelError::TypeError(
                                    loc,
                                    "unquote-splicing requires a list".into(),
                                ));
                            }
                        }
                    }
                    self.stack.push(Value::List(Box::new(items)));
                }
                27 => {
                    // Sum
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Integer(a + b)
                        } else {
                            internal::sum_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::sum_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                28 => {
                    // Sub
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Integer(a - b)
                        } else {
                            internal::sub_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::sub_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                29 => {
                    // Mul
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Integer(a * b)
                        } else {
                            internal::mul_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::mul_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                30 => {
                    // Div
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = internal::div_slice(loc, &self.stack[start..])?;
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                31 => {
                    // Mod
                    let start = self.stack.len() - 2;
                    let res = if let (Value::Integer(a), Value::Integer(b)) =
                        (&self.stack[start], &self.stack[start + 1])
                    {
                        Value::Integer(a % b)
                    } else {
                        internal::modulo_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                32 => {
                    // Eq
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = internal::is_equal_slice(loc, &self.stack[start..])?;
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                33 => {
                    // NumEq
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Boolean(a == b)
                        } else {
                            internal::num_eq_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::num_eq_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                34 => {
                    // NumNotEq
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Boolean(a != b)
                        } else {
                            internal::num_noteq_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::num_noteq_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                35 => {
                    // NumLt
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Boolean(a < b)
                        } else {
                            internal::num_lt_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::num_lt_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                36 => {
                    // NumGt
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Boolean(a > b)
                        } else {
                            internal::num_gt_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::num_gt_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                37 => {
                    // NumLte
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Boolean(a <= b)
                        } else {
                            internal::num_lte_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::num_lte_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                38 => {
                    // NumGte
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = if arity == 2 {
                        if let (Value::Integer(a), Value::Integer(b)) =
                            (&self.stack[start], &self.stack[start + 1])
                        {
                            Value::Boolean(a >= b)
                        } else {
                            internal::num_gte_slice(loc, &self.stack[start..])?
                        }
                    } else {
                        internal::num_gte_slice(loc, &self.stack[start..])?
                    };
                    self.stack.truncate(start);
                    self.stack.push(res);
                }
                39 => {
                    // Cons
                    let start = self.stack.len() - 2;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::cons(loc, args)?);
                }
                40 => {
                    // Car
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::car(loc, vec![arg])?);
                }
                41 => {
                    // Cdr
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::cdr(loc, vec![arg])?);
                }
                42 => {
                    // Nth
                    let start = self.stack.len() - 2;
                    let args: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(internal::nth(loc, args)?);
                }
                43 => {
                    // Count
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::count(loc, vec![arg])?);
                }
                44 => {
                    // Empty
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::empty(loc, vec![arg])?);
                }
                45 => {
                    // IsNil
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::is_nil(loc, vec![arg])?);
                }
                46 => {
                    // IsList
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::is_list(loc, vec![arg])?);
                }
                47 => {
                    // IsNumber
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::is_number(loc, vec![arg])?);
                }
                48 => {
                    // IsString
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::is_string(loc, vec![arg])?);
                }
                49 => {
                    // IsSymbol
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::is_symbol(loc, vec![arg])?);
                }
                50 => {
                    // IsFunction
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::is_function(loc, vec![arg])?);
                }
                51 => {
                    // TypeOf
                    let arg = self.stack.pop().unwrap();
                    self.stack.push(internal::type_of(loc, vec![arg])?);
                }
                52 => {
                    // Not
                    let arity = read_u32(frame) as usize;
                    let start = self.stack.len() - arity;
                    let res = internal::not(loc, self.stack.drain(start..).collect())?;
                    self.stack.push(res);
                }
                53 => {
                    // Load
                    let path_val = self.stack.pop().ok_or_else(|| {
                        SelError::Runtime(frame.loc, "load: missing path on stack".into())
                    })?;
                    if let Some(path_str) = path_val.to_string_lossy() {
                        let fp = PathBuf::from(lookup(loc.file_id));
                        let target_path = if lookup(loc.file_id) != "<repl>"
                            && fp
                                .parent()
                                .is_some_and(|p| p.is_dir() && p != std::path::Path::new(""))
                        {
                            fp.parent().unwrap().join(&path_str)
                        } else {
                            std::env::current_dir().unwrap_or_default().join(&path_str)
                        };

                        if let Some(ref root) = self.sandbox_root {
                            check_sandbox(&target_path, root, loc)?;
                        }

                        let src = read_script(&target_path)
                            .map_err(|e| SelError::Internal(e.to_string()))?;
                        let mut diags = Vec::new();
                        let file_id = intern(target_path.to_string_lossy().as_ref());
                        let asts = parse_all(&src, file_id, &mut diags);
                        if !diags.is_empty() {
                            return Err(diags.remove(0));
                        }

                        let result_val = execute_asts_sandboxed(
                            asts,
                            frame.env.clone(),
                            self.sandbox_root.clone(),
                        )?;
                        self.stack.push(result_val);
                    } else {
                        return Err(SelError::Runtime(
                            frame.loc,
                            format!("load: expected string path, got {:?}", path_val),
                        ));
                    }
                }
                _ => unreachable!(),
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
                    let mut locals = Vec::new();

                    for (i, pid) in params.iter().enumerate() {
                        if lookup(*pid).starts_with('&') {
                            let rest_args = args.split_off(i);
                            let name = &lookup(*pid)[1..];
                            let rest_val = Value::make_list(rest_args);
                            call_env.insert(intern(name), rest_val.clone());
                            locals.push(rest_val);
                            break;
                        } else if i < args.len() {
                            let arg_val = args[i].clone();
                            call_env.insert(*pid, arg_val.clone());
                            locals.push(arg_val);
                        } else {
                            return Err(SelError::ArityMismatch {
                                loc,
                                expected,
                                actual: args.len(),
                            });
                        }
                    }

                    let mut vm = VM::new();
                    let result_val = vm.run(loc, chunk, Rc::new(RefCell::new(call_env)), locals)?;

                    let expanded_ast = value_to_ast(result_val, loc)?;
                    return macro_expand(expanded_ast, env);
                }
                if lookup(id) == "match" {
                    let mut iter = list.into_iter().skip(1);
                    let target = iter.next().ok_or_else(|| {
                        SelError::SyntaxError(loc, "Expected target expression in match".into())
                    })?;
                    let exp_target = macro_expand(target, env.clone())?;
                    let mut exp_list = vec![Ast::Symbol(loc, id), exp_target];
                    for clause in iter {
                        match clause {
                            Ast::List(c_loc, c_items) => {
                                if c_items.is_empty() {
                                    exp_list.push(Ast::List(c_loc, c_items));
                                } else {
                                    let mut exp_clause = Vec::new();
                                    let mut c_iter = c_items.into_iter();
                                    // Pattern is preserved unexpanded
                                    exp_clause.push(c_iter.next().unwrap());
                                    for item in c_iter {
                                        exp_clause.push(macro_expand(item, env.clone())?);
                                    }
                                    exp_list.push(Ast::List(c_loc, exp_clause));
                                }
                            }
                            other => exp_list.push(macro_expand(other, env.clone())?),
                        }
                    }
                    return Ok(Ast::List(loc, exp_list));
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
        Ast::Load(loc, path) => Ok(Ast::Load(loc, Box::new(macro_expand(*path, env)?))),
        Ast::Match(loc, target, clauses) => {
            let exp_target = macro_expand(*target, env.clone())?;
            let mut exp_clauses = Vec::with_capacity(clauses.len());
            for c in clauses {
                let exp_guard = match c.guard {
                    Some(g) => Some(macro_expand(g, env.clone())?),
                    None => None,
                };
                let mut exp_body = Vec::with_capacity(c.body.len());
                for b in c.body {
                    exp_body.push(macro_expand(b, env.clone())?);
                }
                exp_clauses.push(crate::ast::MatchClause {
                    loc: c.loc,
                    pattern: c.pattern,
                    guard: exp_guard,
                    body: exp_body,
                });
            }
            Ok(Ast::Match(loc, Box::new(exp_target), exp_clauses))
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
        Ast::Load(loc, path) => Ok(Ast::Load(
            loc,
            Box::new(macro_expand_quasiquote(*path, env)?),
        )),
        _ => Ok(ast),
    }
}

pub fn execute_asts_sandboxed(
    asts: Vec<Ast>,
    env: Rc<RefCell<Env>>,
    sandbox_root: Option<PathBuf>,
) -> Result<Value> {
    let mut last_val = Value::Nil;
    let mut vm = VM::new();
    vm.sandbox_root = sandbox_root;
    for ast in asts {
        let loc = ast.loc();
        let expanded = macro_expand(ast, env.clone())?;
        let resolved = crate::parser::resolve_ast(expanded)?;
        let mut chunk = Chunk::new();
        let mut compiler = Compiler::new(&mut chunk);
        compiler.compile(resolved)?;
        last_val = vm.run(loc, Rc::new(chunk), env.clone(), Vec::new())?;
    }
    Ok(last_val)
}

pub fn execute_asts_debug(asts: Vec<Ast>, env: Rc<RefCell<Env>>) -> Result<Value> {
    execute_asts_sandboxed(asts, env, None)
}

pub fn execute_asts(asts: Vec<Ast>, env: Rc<RefCell<Env>>) -> Result<Value> {
    execute_asts_sandboxed(asts, env, None)
}

pub fn import_module_sandboxed(
    module_name: &str,
    asts: Vec<Ast>,
    env: Rc<RefCell<Env>>,
    sandbox_root: Option<PathBuf>,
) -> Result<Record<Value>> {
    let mut file_record = Record::new();
    execute_asts_sandboxed(asts, env.clone(), sandbox_root)?;
    for (sym, value) in env.borrow().bindings.iter() {
        if env.borrow().private_bindings.contains(sym) {
            continue;
        }
        file_record.fields_mut().insert(
            intern(&format!("{module_name}/{}", lookup(*sym))),
            value.clone(),
        );
    }
    Ok(file_record)
}

pub fn import_module(
    module_name: &str,
    asts: Vec<Ast>,
    env: Rc<RefCell<Env>>,
) -> Result<Record<Value>> {
    import_module_sandboxed(module_name, asts, env, None)
}
