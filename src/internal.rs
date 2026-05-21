use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::diagnostics::*;
use crate::lexer::Loc;
use crate::parser::parse_all;
use crate::runtime::Env;
use crate::runtime::execute_asts;
use crate::types::intern;
use crate::types::lookup;
use crate::value::*;

type Result<T> = std::result::Result<T, SelError>;

#[inline]
pub fn sum(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let mut int_sum = 0;
    let mut float_sum = 0.0;
    let mut is_float = false;

    for arg in args {
        match arg {
            Value::Integer(i) => {
                if is_float {
                    float_sum += i as f64;
                } else {
                    int_sum += i;
                }
            }
            Value::Float(f) => {
                if !is_float {
                    is_float = true;
                    float_sum = int_sum as f64 + f;
                } else {
                    float_sum += f;
                }
            }
            v => {
                return Err(SelError::TypeError(
                    loc,
                    format!(
                        "Invalid argument to +: expected number but got {}",
                        value_type_name(&v)
                    ),
                ));
            }
        }
    }
    if is_float {
        Ok(Value::Float(float_sum))
    } else {
        Ok(Value::Integer(int_sum))
    }
}

#[inline]
pub fn sub(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let mut is_float = false;
    let mut int_val = 0;
    let mut float_val = 0.0;

    match &args[0] {
        Value::Integer(i) => int_val = *i,
        Value::Float(f) => {
            is_float = true;
            float_val = *f;
        }
        v => {
            return Err(SelError::TypeError(
                loc,
                format!(
                    "Invalid argument to -: expected number but got {}",
                    value_type_name(v)
                ),
            ));
        }
    }

    if args.len() == 1 {
        return if is_float {
            Ok(Value::Float(-float_val))
        } else {
            Ok(Value::Integer(-int_val))
        };
    }

    for arg in args.into_iter().skip(1) {
        match arg {
            Value::Integer(i) => {
                if is_float {
                    float_val -= i as f64;
                } else {
                    int_val -= i;
                }
            }
            Value::Float(f) => {
                if !is_float {
                    is_float = true;
                    float_val = int_val as f64 - f;
                } else {
                    float_val -= f;
                }
            }
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to -: expected number".into(),
                ));
            }
        }
    }
    if is_float {
        Ok(Value::Float(float_val))
    } else {
        Ok(Value::Integer(int_val))
    }
}

#[inline]
pub fn mul(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let mut int_val = 1;
    let mut float_val = 1.0;
    let mut is_float = false;

    for arg in args {
        match arg {
            Value::Integer(i) => {
                if is_float {
                    float_val *= i as f64;
                } else {
                    int_val *= i;
                }
            }
            Value::Float(f) => {
                if !is_float {
                    is_float = true;
                    float_val = int_val as f64 * f;
                } else {
                    float_val *= f;
                }
            }
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to *: expected number".into(),
                ));
            }
        }
    }
    if is_float {
        Ok(Value::Float(float_val))
    } else {
        Ok(Value::Integer(int_val))
    }
}

pub fn div(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() == 1 {
        match args[0] {
            Value::Integer(i) => return Ok(Value::Float(1.0 / i as f64)),
            Value::Float(f) => return Ok(Value::Float(1.0 / f)),
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to /: expected number".into(),
                ));
            }
        }
    }

    let mut float_val = match args[0] {
        Value::Integer(i) => i as f64,
        Value::Float(f) => f,
        _ => {
            return Err(SelError::Runtime(
                loc,
                "Invalid argument to /: expected number".into(),
            ));
        }
    };

    for arg in args.into_iter().skip(1) {
        match arg {
            Value::Integer(i) => float_val /= i as f64,
            Value::Float(f) => float_val /= f,
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to /: expected number".into(),
                ));
            }
        }
    }
    Ok(Value::Float(float_val))
}

pub fn modulo(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let a = match args[0] {
        Value::Integer(i) => i,
        _ => {
            return Err(SelError::Runtime(loc, "modulo requires integer".into()));
        }
    };
    let b = match args[1] {
        Value::Integer(i) => i,
        _ => {
            return Err(SelError::Runtime(loc, "modulo requires integer".into()));
        }
    };
    Ok(Value::Integer(a % b))
}

fn compare_nums(loc: Loc, args: Vec<Value>, op: fn(f64, f64) -> bool) -> Result<Value> {
    let mut prev = match args[0] {
        Value::Integer(i) => i as f64,
        Value::Float(f) => f,
        _ => {
            return Err(SelError::Runtime(loc, "comparison requires numbers".into()));
        }
    };
    for arg in args.into_iter().skip(1) {
        let curr = match arg {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => {
                return Err(SelError::Runtime(loc, "comparison requires numbers".into()));
            }
        };
        if !op(prev, curr) {
            return Ok(Value::Boolean(false));
        }
        prev = curr;
    }
    Ok(Value::Boolean(true))
}

pub fn is_equal(_loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Boolean(true));
    }
    let first = &args[0];
    for arg in args.iter().skip(1) {
        let eq = is_value_equal(first, arg);
        if !eq {
            return Ok(Value::Boolean(false));
        }
    }
    Ok(Value::Boolean(true))
}

fn is_value_equal(first: &Value, arg: &Value) -> bool {
    match (first, arg) {
        (Value::Nil, Value::Nil) => true,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Symbol(a), Value::Symbol(b)) => a == b,
        (Value::Pointer(a), Value::Pointer(b)) => a == b,
        (Value::List(a), Value::List(b)) => {
            a.iter().zip(b.iter()).all(|(a, b)| is_value_equal(a, b))
        }
        (Value::Record(a), Value::Record(b)) => a
            .fields()
            .iter()
            .zip(b.fields())
            .all(|((ka, va), (kb, vb))| *ka == *kb && is_value_equal(va, vb)),
        _ => false,
    }
}

pub fn num_noteq(loc: Loc, args: Vec<Value>) -> Result<Value> {
    compare_nums(loc, args, |a, b| a != b)
}
pub fn num_eq(loc: Loc, args: Vec<Value>) -> Result<Value> {
    compare_nums(loc, args, |a, b| a == b)
}
pub fn num_lt(loc: Loc, args: Vec<Value>) -> Result<Value> {
    compare_nums(loc, args, |a, b| a < b)
}
pub fn num_gt(loc: Loc, args: Vec<Value>) -> Result<Value> {
    compare_nums(loc, args, |a, b| a > b)
}
pub fn num_lte(loc: Loc, args: Vec<Value>) -> Result<Value> {
    compare_nums(loc, args, |a, b| a <= b)
}
pub fn num_gte(loc: Loc, args: Vec<Value>) -> Result<Value> {
    compare_nums(loc, args, |a, b| a >= b)
}

pub fn error(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let msg = args
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    Err(SelError::Runtime(loc, msg))
}

pub fn value_type_name(v: &Value) -> &str {
    match v {
        Value::Nil => "nil",
        Value::Integer(_) => "int",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Boolean(_) => "bool",
        Value::Symbol(_) => "symbol",
        Value::List(_) => "list",
        Value::NativeClosure(_) | Value::Closure(_) | Value::NativeFunction(_) => "function",
        Value::Macro { .. } => "macro",
        Value::Pointer(_) => "pointer",
        Value::Library(_) => "library",
        Value::Record(_) => "record",
        Value::Coroutine(_) => "coroutine",
    }
}

pub fn not(_loc: Loc, args: Vec<Value>) -> Result<Value> {
    match args[0] {
        Value::Boolean(false) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn display_newline(loc: Loc, args: Vec<Value>) -> Result<Value> {
    display(loc, args.clone())?;
    println!();
    Ok(Value::Nil)
}

pub fn display(_loc: Loc, args: Vec<Value>) -> Result<Value> {
    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{}", arg);
    }
    use std::io::Write;
    std::io::stdout().flush().unwrap();
    Ok(Value::Nil)
}

pub fn ffi_dlopen(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for ffi-dlopen".into(),
        ));
    }
    if let Value::String(s) = &args[0] {
        unsafe {
            match libloading::Library::new(s.as_str()) {
                Ok(lib) => Ok(Value::Library(Rc::new(lib))),
                Err(e) => Err(SelError::Runtime(loc, format!("dlopen failed: {}", e))),
            }
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "ffi-dlopen requires a string".into(),
        ))
    }
}

pub fn ffi_dlsym(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for ffi-dlsym".into(),
        ));
    }
    let lib = match &args[0] {
        Value::Library(l) => l,
        _ => {
            return Err(SelError::Runtime(
                loc,
                "ffi-dlsym requires a library".into(),
            ));
        }
    };
    let sym_name = match &args[1] {
        Value::String(s) => s,
        _ => {
            return Err(SelError::Runtime(
                loc,
                "ffi-dlsym requires a string symbol name".into(),
            ));
        }
    };

    let mut sym_bytes = sym_name.as_bytes().to_vec();
    sym_bytes.push(0);

    unsafe {
        match lib.get::<*const ()>(&sym_bytes) {
            Ok(sym) => {
                let ptr = *sym as usize;
                Ok(Value::Pointer(ptr))
            }
            Err(e) => Err(SelError::Runtime(loc, format!("dlsym failed: {}", e))),
        }
    }
}

pub fn ffi_call(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 4 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 4 arguments for ffi-call".into(),
        ));
    }
    let ptr = match args[0] {
        Value::Pointer(p) => p,
        _ => {
            return Err(SelError::Runtime(loc, "ffi-call requires a pointer".into()));
        }
    };

    let ret_type_sym = match args[1] {
        Value::Symbol(s) => crate::lookup(s),
        _ => {
            return Err(SelError::Runtime(
                loc,
                "ffi-call requires a return type symbol".into(),
            ));
        }
    };

    let arg_type_syms = match &args[2] {
        Value::List(l) => {
            let mut syms = Vec::new();
            for v in l.iter() {
                if let Value::Symbol(s) = v {
                    syms.push(crate::lookup(*s));
                } else {
                    return Err(SelError::Runtime(
                        loc,
                        format!("arg_types must be a list of symbols {v}"),
                    ));
                }
            }
            syms
        }
        Value::Nil => Vec::new(),
        _ => {
            return Err(SelError::Runtime(loc, "arg_types must be a list".into()));
        }
    };

    let arg_vals = match args[3].clone() {
        Value::List(l) => l,
        Value::Nil => Rc::new(Vec::new()),
        _ => {
            return Err(SelError::Runtime(loc, "arg_vals must be a list".into()));
        }
    };

    let ret_type = match ret_type_sym.as_str() {
        "void" => libffi::middle::Type::void(),
        "i32" => libffi::middle::Type::i32(),
        "i64" => libffi::middle::Type::i64(),
        "u32" => libffi::middle::Type::u32(),
        "u64" => libffi::middle::Type::u64(),
        "f32" => libffi::middle::Type::f32(),
        "f64" => libffi::middle::Type::f64(),
        "bool" => libffi::middle::Type::u8(),
        "*u8" => libffi::middle::Type::pointer(),
        _ => {
            return Err(SelError::Runtime(
                loc,
                format!("Unsupported return type: {}", ret_type_sym),
            ));
        }
    };

    let mut arg_types = Vec::new();
    for sym in &arg_type_syms {
        let t = match sym.as_str() {
            "i32" => libffi::middle::Type::i32(),
            "i64" => libffi::middle::Type::i64(),
            "u32" => libffi::middle::Type::u32(),
            "u64" => libffi::middle::Type::u64(),
            "f32" => libffi::middle::Type::f32(),
            "f64" => libffi::middle::Type::f64(),
            "bool" => libffi::middle::Type::u8(),
            "*u8" => libffi::middle::Type::pointer(),
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    format!("Unsupported arg type: {}", sym),
                ));
            }
        };
        arg_types.push(t);
    }

    let cif = libffi::middle::Cif::new(arg_types, ret_type);

    let mut c_strings = Vec::new();

    enum FfiArg {
        I32(i32),
        I64(i64),
        U8(u8),
        U32(u32),
        U64(u64),
        F32(f32),
        F64(f64),
        Ptr(*const std::ffi::c_void),
    }

    let mut ffi_args_storage = Vec::new();

    for (i, arg_val) in arg_vals.iter().enumerate() {
        let sym: &str = &arg_type_syms[i];
        match sym {
            "i32" => {
                let v = match arg_val {
                    Value::Integer(n) => *n as i32,
                    Value::Float(f) => *f as i32,
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected integer for arg {}", i),
                        ));
                    }
                };
                ffi_args_storage.push(FfiArg::I32(v));
            }
            "bool" => {
                let v = match arg_val {
                    Value::Boolean(b) => {
                        if *b {
                            1u8
                        } else {
                            0u8
                        }
                    }
                    Value::Integer(n) => {
                        if *n != 0 {
                            1u8
                        } else {
                            0u8
                        }
                    }
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected boolean for arg {}", i),
                        ));
                    }
                };
                ffi_args_storage.push(FfiArg::U8(v));
            }
            "i64" => {
                let v = match arg_val {
                    Value::Integer(n) => *n,
                    Value::Float(f) => *f as i64,
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected integer for arg {}", i),
                        ));
                    }
                };
                ffi_args_storage.push(FfiArg::I64(v));
            }
            "u32" => {
                let v = match arg_val {
                    Value::Integer(n) => *n as u32,
                    Value::Float(f) => *f as u32,
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected integer for arg {}", i),
                        ));
                    }
                };
                ffi_args_storage.push(FfiArg::U32(v));
            }
            "u64" => {
                let v = match arg_val {
                    Value::Integer(n) => *n as u64,
                    Value::Float(f) => *f as u64,
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected integer for arg {}", i),
                        ));
                    }
                };
                ffi_args_storage.push(FfiArg::U64(v));
            }
            "f32" => {
                let v = match arg_val {
                    Value::Integer(n) => *n as f32,
                    Value::Float(f) => *f as f32,
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected float for arg {}", i),
                        ));
                    }
                };
                ffi_args_storage.push(FfiArg::F32(v));
            }
            "f64" => {
                let v = match arg_val {
                    Value::Integer(n) => *n as f64,
                    Value::Float(f) => *f,
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected float for arg {}", i),
                        ));
                    }
                };
                ffi_args_storage.push(FfiArg::F64(v));
            }
            "*u8" => {
                match arg_val {
                    Value::String(s) => {
                        let cstr = std::ffi::CString::new(s.as_str()).unwrap();
                        let ptr = cstr.as_ptr() as *const std::ffi::c_void;
                        c_strings.push(cstr); // keep alive
                        ffi_args_storage.push(FfiArg::Ptr(ptr));
                    }
                    Value::Pointer(p) => {
                        ffi_args_storage.push(FfiArg::Ptr(*p as *const std::ffi::c_void));
                    }
                    Value::Nil => {
                        ffi_args_storage.push(FfiArg::Ptr(std::ptr::null()));
                    }
                    _ => {
                        return Err(SelError::Runtime(
                            loc,
                            format!("Expected string or pointer for arg {}", i),
                        ));
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    let mut call_args = Vec::new();
    for arg in &ffi_args_storage {
        match arg {
            FfiArg::I32(v) => call_args.push(libffi::middle::arg(v)),
            FfiArg::I64(v) => call_args.push(libffi::middle::arg(v)),
            FfiArg::U8(v) => call_args.push(libffi::middle::arg(v)),
            FfiArg::U32(v) => call_args.push(libffi::middle::arg(v)),
            FfiArg::U64(v) => call_args.push(libffi::middle::arg(v)),
            FfiArg::F32(v) => call_args.push(libffi::middle::arg(v)),
            FfiArg::F64(v) => call_args.push(libffi::middle::arg(v)),
            FfiArg::Ptr(v) => call_args.push(libffi::middle::arg(v)),
        }
    }

    let code_ptr = libffi::middle::CodePtr::from_ptr(ptr as *mut _);

    unsafe {
        match ret_type_sym.as_str() {
            "void" => {
                cif.call::<()>(code_ptr, &call_args);
                Ok(Value::Nil)
            }
            "bool" => {
                let res: u8 = cif.call(code_ptr, &call_args);
                Ok(Value::Boolean(res != 0))
            }
            "i32" => {
                let res: i32 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            "i64" => {
                let res: i64 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res))
            }
            "u32" => {
                let res: u32 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            "u64" => {
                let res: u64 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            "f32" => {
                let res: f32 = cif.call(code_ptr, &call_args);
                Ok(Value::Float(res as f64))
            }
            "f64" => {
                let res: f64 = cif.call(code_ptr, &call_args);
                Ok(Value::Float(res))
            }
            "*u8" => {
                let res: *const std::ffi::c_char = cif.call(code_ptr, &call_args);
                if res.is_null() {
                    Ok(Value::Nil)
                } else {
                    let c_str = std::ffi::CStr::from_ptr(res);
                    Ok(Value::String(Rc::new(c_str.to_string_lossy().into_owned())))
                }
            }
            _ => unreachable!(),
        }
    }
}

pub fn cons(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 2 arguments for cons".into(),
        ));
    }
    let tail = args.pop().unwrap();
    let head = args.pop().unwrap();
    match tail {
        Value::List(l) => {
            let mut new_l = vec![head];
            new_l.extend(l.iter().cloned());
            Ok(Value::List(Rc::new(new_l)))
        }
        Value::Nil => Ok(Value::List(Rc::new(vec![head]))),
        _ => Ok(Value::List(Rc::new(vec![head, tail]))),
    }
}

pub fn car(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for car".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(l) => {
            if l.is_empty() {
                return Err(SelError::Runtime(loc, "car on empty list".into()));
            }
            Ok(l[0].clone())
        }
        _ => Err(SelError::Runtime(loc, "car requires a list".into())),
    }
}

pub fn cdr(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for cdr".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(l) => {
            if l.is_empty() {
                return Err(SelError::Runtime(loc, "cdr on empty list".into()));
            }
            if l.len() == 1 {
                return Ok(Value::Nil);
            }
            let mut new_l = Vec::with_capacity(l.len() - 1);
            new_l.extend_from_slice(&l[1..]);
            Ok(Value::List(Rc::new(new_l)))
        }
        _ => Err(SelError::Runtime(loc, "cdr requires a list".into())),
    }
}

pub fn nth(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for nth".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::List(l) => match index {
            Value::Integer(index) => Ok(if (index as usize) < l.len() {
                l[index as usize].clone()
            } else {
                Value::Nil
            }),
            _ => Err(SelError::Runtime(loc, "nth requires a interger".into())),
        },
        _ => Err(SelError::Runtime(loc, "nth requires a list".into())),
    }
}

pub fn count(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for count".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(l) => Ok(Value::Integer(l.len() as _)),
        Value::String(s) => Ok(Value::Integer(s.len() as _)),
        Value::Nil => Ok(Value::Integer(0)),
        _ => Err(SelError::Runtime(loc, "count requires a list".into())),
    }
}

pub fn list(_loc: Loc, args: Vec<Value>) -> Result<Value> {
    Ok(Value::List(Rc::new(args)))
}

pub fn empty(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 1 arguments for empty?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(l) => Ok(Value::Boolean(l.is_empty())),
        Value::Nil => Ok(Value::Boolean(true)),
        Value::String(s) => Ok(Value::Boolean(s.is_empty())),
        v => Err(SelError::Runtime(
            loc,
            format!("empty requires a list got {v}"),
        )),
    }
}

pub fn rget(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for rget".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => Ok(if let Some(v) = r.fields().get(&sym).cloned() {
                v
            } else {
                Value::Nil
            }),
            _ => Err(SelError::Runtime(loc, "rget requires a symbol".into())),
        },
        _ => Err(SelError::Runtime(loc, "rget requires a record".into())),
    }
}

pub fn rset(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 3 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 3 arguments for rset".into(),
        ));
    }
    let value = args.pop().unwrap();
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => {
                let mut new_r = (*r).clone();
                new_r.fields_mut().insert(sym, value);
                Ok(Value::Record(Rc::new(new_r)))
            }
            _ => {
                return Err(SelError::Runtime(loc, "rset requires a symbol".into()));
            }
        },
        _ => Err(SelError::Runtime(loc, "rset requires a record".into())),
    }
}

pub fn rdel(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for rdel".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => {
                let mut new_r = (*r).clone();
                new_r.fields_mut().shift_remove(&sym);
                Ok(Value::Record(Rc::new(new_r)))
            }
            _ => Err(SelError::Runtime(loc, "rdel requires a symbol".into())),
        },
        _ => Err(SelError::Runtime(loc, "rdel requires a record".into())),
    }
}

pub fn rkeys(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 argument for rkeys".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Record(r) => {
            let keys_vec: Vec<Value> = r
                .fields()
                .keys()
                .map(|&k| Value::Symbol(k))
                .collect();
            Ok(Value::List(Rc::new(keys_vec)))
        }
        _ => Err(SelError::Runtime(loc, "rkeys requires a record".into())),
    }
}

pub fn rvals(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 argument for rvals".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Record(r) => {
            let vals_vec: Vec<Value> = r
                .fields()
                .values()
                .cloned()
                .collect();
            Ok(Value::List(Rc::new(vals_vec)))
        }
        _ => Err(SelError::Runtime(loc, "rvals requires a record".into())),
    }
}

pub fn rcontains(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for rcontains?".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => Ok(Value::Boolean(r.fields().contains_key(&sym))),
            _ => Err(SelError::Runtime(loc, "rcontains? requires a symbol".into())),
        },
        _ => Err(SelError::Runtime(loc, "rcontains? requires a record".into())),
    }
}


pub fn is_nil(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for nil?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Nil => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_list(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for list?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(_) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_number(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for number?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Integer(_) => Ok(Value::Boolean(true)),
        Value::Float(_) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_string(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for string?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::String(_) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_symbol(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for symbol?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Symbol(_) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_record(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for record?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Record(_) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_function(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for function?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Closure(_) => Ok(Value::Boolean(true)),
        Value::NativeFunction(_) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn type_of(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for type-of".into(),
        ));
    }
    let v = args.pop().unwrap();
    Ok(Value::Symbol(intern(value_type_name(&v))))
}

pub fn newline(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if !args.is_empty() {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for newline".into(),
        ));
    }
    println!();
    Ok(Value::Nil)
}

pub fn file_system(loc: Loc, mut call_args: Vec<Value>) -> Result<Value> {
    let args = call_args.split_off(1);
    if let Some(Value::Symbol(sym)) = call_args.pop() {
        match lookup(sym).as_str() {
            "exists?" => {
                if args.len() != 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 1 arguments for file-exists?".into(),
                    ));
                }
                if let Value::String(path) = &args[0] {
                    Ok(Value::Boolean(std::path::Path::new(path.as_str()).exists()))
                } else {
                    Err(SelError::Runtime(
                        loc,
                        "file-exists? requires a string argument".into(),
                    ))
                }
            }
            "write" => fs_write(loc, &args),
            "read" => fs_read(loc, &args),
            _ => todo!(),
        }
    } else {
        return Err(SelError::SyntaxError(
            loc,
            "Expected symbol for system".into(),
        ));
    }
}

fn fs_write(loc: Loc, args: &Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 2 arguments for write".into(),
        ));
    }
    if let (Value::String(path), Value::String(content)) = (&args[0], &args[1]) {
        match std::fs::write(path.as_str(), content.as_str()) {
            Ok(_) => Ok(Value::Nil),
            Err(e) => Err(SelError::Runtime(loc, format!("write failed: {}", e))),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "write requires string arguments".into(),
        ))
    }
}

fn fs_read(loc: Loc, args: &Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 1 arguments for read".into(),
        ));
    }
    if let Value::String(path) = &args[0] {
        match std::fs::read_to_string(path.as_str()) {
            Ok(content) => Ok(Value::String(Rc::new(content))),
            Err(e) => Err(SelError::Runtime(loc, format!("read failed: {}", e))),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "read requires a string argument".into(),
        ))
    }
}

pub fn system(loc: Loc, mut system_args: Vec<Value>) -> Result<Value> {
    let mut args = system_args.split_off(1);
    if let Some(Value::Symbol(sym)) = system_args.pop() {
        match lookup(sym).as_str() {
            "args" => {
                if args.len() != 0 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 0 arguments for args".into(),
                    ));
                }
                let args_vec = std::env::args()
                    .skip(1)
                    .map(|s| Value::String(Rc::new(s)))
                    .collect::<Vec<_>>();
                Ok(Value::List(Rc::new(args_vec)))
            }
            "getenv" => {
                if args.len() != 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 1 arguments for getenv".into(),
                    ));
                }
                if let Value::String(key) = &args[0] {
                    match std::env::var(key.as_str()) {
                        Ok(val) => Ok(Value::String(Rc::new(val))),
                        Err(_) => Ok(Value::Nil),
                    }
                } else {
                    Err(SelError::Runtime(
                        loc,
                        "getenv requires a string argument".into(),
                    ))
                }
            }
            "exit" => {
                if args.len() > 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 0 or 1 arguments for exit".into(),
                    ));
                }
                let code = match args.pop() {
                    Some(Value::Integer(code)) => code as _,
                    _ => 0,
                };
                std::process::exit(code)
            }
            "sleep" => {
                if args.len() != 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 1 arguments for sleep".into(),
                    ));
                }
                if let Value::Integer(d) = &args[0] {
                    std::thread::sleep(std::time::Duration::from_secs(*d as _));
                    Ok(Value::Nil)
                } else {
                    Err(SelError::Runtime(
                        loc,
                        "getenv requires a string argument".into(),
                    ))
                }
            }
            _ => todo!(),
        }
    } else {
        return Err(SelError::SyntaxError(
            loc,
            "Expected symbol for system".into(),
        ));
    }
}

pub fn co_create(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "co-create requires exactly 1 argument".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Closure(closure) => {
            let co = Coroutine {
                state: std::cell::Cell::new(CoroutineState::Suspended),
                frames: RefCell::new(Vec::new()),
                operand_stack: RefCell::new(Vec::new()),
                closure,
            };
            Ok(Value::Coroutine(Rc::new(co)))
        }
        val => Err(SelError::Runtime(
            loc,
            format!("co-create: expected closure but got {}", val),
        )),
    }
}

pub fn co_state(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "co-state requires exactly 1 argument".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Coroutine(co) => {
            let state_str = match co.state.get() {
                CoroutineState::Suspended => "suspended",
                CoroutineState::Running => "running",
                CoroutineState::Dead => "dead",
            };
            Ok(Value::Symbol(intern(state_str)))
        }
        val => Err(SelError::Runtime(
            loc,
            format!("co-state: expected coroutine but got {}", val),
        )),
    }
}

pub fn co_dead_p(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "co-dead? requires exactly 1 argument".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Coroutine(co) => {
            Ok(Value::Boolean(co.state.get() == CoroutineState::Dead))
        }
        val => Err(SelError::Runtime(
            loc,
            format!("co-dead?: expected coroutine but got {}", val),
        )),
    }
}

pub fn load(env: Rc<RefCell<Env>>) {
    let mut e = env.borrow_mut();
    e.insert(intern("+"), Value::NativeFunction(sum));
    e.insert(intern("-"), Value::NativeFunction(sub));
    e.insert(intern("*"), Value::NativeFunction(mul));
    e.insert(intern("/"), Value::NativeFunction(div));
    e.insert(intern("mod"), Value::NativeFunction(modulo));

    e.insert(intern("not"), Value::NativeFunction(not));
    e.insert(intern("eq?"), Value::NativeFunction(is_equal));
    e.insert(intern("="), Value::NativeFunction(num_eq));
    e.insert(intern("!="), Value::NativeFunction(num_noteq));
    e.insert(intern("<"), Value::NativeFunction(num_lt));
    e.insert(intern(">"), Value::NativeFunction(num_gt));
    e.insert(intern("<="), Value::NativeFunction(num_lte));
    e.insert(intern(">="), Value::NativeFunction(num_gte));

    e.insert(intern("cons"), Value::NativeFunction(cons));
    e.insert(intern("car"), Value::NativeFunction(car));
    e.insert(intern("cdr"), Value::NativeFunction(cdr));
    e.insert(intern("nth"), Value::NativeFunction(nth));
    e.insert(intern("count"), Value::NativeFunction(count));
    e.insert(intern("list"), Value::NativeFunction(list));
    e.insert(intern("empty?"), Value::NativeFunction(empty));

    e.insert(intern("rget"), Value::NativeFunction(rget));
    e.insert(intern("rset"), Value::NativeFunction(rset));
    e.insert(intern("rdel"), Value::NativeFunction(rdel));
    e.insert(intern("rkeys"), Value::NativeFunction(rkeys));
    e.insert(intern("rvals"), Value::NativeFunction(rvals));
    e.insert(intern("rcontains?"), Value::NativeFunction(rcontains));

    e.insert(intern("nil?"), Value::NativeFunction(is_nil));
    e.insert(intern("list?"), Value::NativeFunction(is_list));
    e.insert(intern("number?"), Value::NativeFunction(is_number));
    e.insert(intern("string?"), Value::NativeFunction(is_string));
    e.insert(intern("symbol?"), Value::NativeFunction(is_symbol));
    e.insert(intern("function?"), Value::NativeFunction(is_function));
    e.insert(intern("record?"), Value::NativeFunction(is_record));

    e.insert(intern("type-of"), Value::NativeFunction(type_of));

    e.insert(intern("error"), Value::NativeFunction(error));
    e.insert(intern("display"), Value::NativeFunction(display));
    e.insert(intern("println"), Value::NativeFunction(display_newline));
    e.insert(intern("newline"), Value::NativeFunction(newline));

    e.insert(intern("ffi-dlopen"), Value::NativeFunction(ffi_dlopen));
    e.insert(intern("ffi-dlsym"), Value::NativeFunction(ffi_dlsym));
    e.insert(intern("ffi-call"), Value::NativeFunction(ffi_call));

    e.insert(intern("system"), Value::NativeFunction(system));
    e.insert(intern("file-system"), Value::NativeFunction(file_system));

    e.insert(intern("co-create"), Value::NativeFunction(co_create));
    e.insert(intern("co-state"), Value::NativeFunction(co_state));
    e.insert(intern("co-dead?"), Value::NativeFunction(co_dead_p));
}

pub fn read_script<P>(script_path: P) -> Result<String>
where
    P: AsRef<Path>,
{
    let mut src =
        std::fs::read_to_string(script_path).map_err(|e| SelError::Internal(e.to_string()))?;
    if src.starts_with("#!") {
        if let Some(newline_idx) = src.find('\n') {
            src = src[newline_idx + 1..].to_string();
        } else {
            src = String::new();
        }
    }
    Ok(src)
}

pub fn load_core_lib() -> Rc<RefCell<Env>> {
    let env = Rc::new(RefCell::new(Env::default()));
    load(env.clone());
    // Load core library if exists
    {
        let core_src = include_str!("core.scm");

        match parse_all(core_src, intern("<core>")) {
            Ok(asts) => {
                if let Err(e) = execute_asts(asts, env.clone()) {
                    eprintln!("Error loading core.scm: {}", e);
                }
            }
            Err(e) => eprintln!("Error parsing core.scm: {}", e),
        }
    }
    env
}
