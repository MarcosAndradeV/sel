use std::cell::RefCell;
use std::rc::Rc;

use crate::compiler::*;
use crate::diagnostics::*;
use crate::lexer::Loc;
use crate::runtime::*;
use crate::types::intern;

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
                    value_type_name(&v)
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
            return Err(SelError::Runtime(
                loc,
                "comparison requires numbers".into(),
            ));
        }
    };
    for arg in args.into_iter().skip(1) {
        let curr = match arg {
            Value::Integer(i) => i as f64,
            Value::Float(f) => f,
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "comparison requires numbers".into(),
                ));
            }
        };
        if !op(prev, curr) {
            return Ok(Value::Boolean(false));
        }
        prev = curr;
    }
    Ok(Value::Boolean(true))
}

pub fn is_equal(loc: Loc, args: Vec<Value>) -> Result<Value> {
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
        (Value::List(a), Value::List(b)) => a.iter().zip(b).all(|(a, b)| is_value_equal(a, b)),
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

pub fn cons(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 2,
            actual: args.len(),
        });
    }
    let tail = args.pop().unwrap();
    let head = args.pop().unwrap();
    match tail {
        Value::List(l) => {
            let mut new_l = vec![head];
            new_l.extend(l);
            Ok(Value::List(new_l))
        }
        Value::Nil => Ok(Value::List(vec![head])),
        _ => Ok(Value::List(vec![head, tail])),
    }
}

pub fn car(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args.pop().unwrap() {
        Value::List(mut l) => {
            if l.is_empty() {
                return Err(SelError::Runtime(
                    loc,
                    "car on empty list".into(),
                ));
            }
            Ok(l.remove(0))
        }
        _ => Err(SelError::Runtime(
            loc,
            "car requires a list".into(),
        )),
    }
}

pub fn nth(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 2,
            actual: args.len(),
        });
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::List(mut l) => match index {
            Value::Integer(index) => {
                if (index as usize) < l.len() {
                    Ok(l.remove(index as usize))
                } else {
                    Ok(Value::Nil)
                }
            }
            _ => Err(SelError::Runtime(
                loc,
                "nth requires a interger".into(),
            )),
        },
        _ => Err(SelError::Runtime(
            loc,
            "nth requires a list".into(),
        )),
    }
}

pub fn cdr(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args.pop().unwrap() {
        Value::List(mut l) => {
            if l.is_empty() {
                return Err(SelError::Runtime(
                    loc,
                    "cdr on empty list".into(),
                ));
            }
            l.remove(0);
            Ok(if l.is_empty() {
                Value::Nil
            } else {
                Value::List(l)
            })
        }
        _ => Err(SelError::Runtime(
            loc,
            "cdr requires a list".into(),
        )),
    }
}

pub fn count(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args.pop().unwrap() {
        Value::List(l) => Ok(Value::Integer(l.len() as _)),
        Value::String(s) => Ok(Value::Integer(s.len() as _)),
        Value::Nil => Ok(Value::Integer(0)),
        _ => Err(SelError::Runtime(
            loc,
            "count requires a list".into(),
        )),
    }
}

pub fn empty(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args.pop().unwrap() {
        Value::List(l) => Ok(Value::Boolean(l.is_empty())),
        Value::Nil => Ok(Value::Boolean(true)),
        _ => Err(SelError::Runtime(
            loc,
            "empty requires a list".into(),
        )),
    }
}

pub fn list(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.is_empty() {
        Ok(Value::Nil)
    } else {
        Ok(Value::List(args))
    }
}

pub fn is_nil(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match &args[0] {
        Value::Nil => Ok(Value::Boolean(true)),
        Value::List(l) if l.is_empty() => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_list(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match &args[0] {
        Value::List(l) if !l.is_empty() => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

pub fn is_number(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    Ok(Value::Boolean(matches!(
        args[0],
        Value::Integer(_) | Value::Float(_)
    )))
}

pub fn is_string(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    Ok(Value::Boolean(matches!(args[0], Value::String(_))))
}

pub fn is_function(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    Ok(Value::Boolean(matches!(
        args[0],
        Value::NativeFunction(_) | Value::Closure { .. }
    )))
}

pub fn is_symbol(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    Ok(Value::Boolean(matches!(args[0], Value::Symbol(_))))
}

pub fn error(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let msg = args
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    Err(SelError::Runtime(loc, msg))
}

pub fn type_of(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    Ok(Value::Symbol(intern(value_type_name(&args[0]))))
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
        Value::NativeFunction(_) | Value::Closure { .. } => "function",
        Value::Macro { .. } => "macro",
        Value::Pointer(_) => "pointer",
        Value::Library(_) => "library",
    }
}

pub fn not(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
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

pub fn display(loc: Loc, args: Vec<Value>) -> Result<Value> {
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

pub fn newline(loc: Loc, _args: Vec<Value>) -> Result<Value> {
    println!();
    Ok(Value::Nil)
}

pub fn ffi_dlopen(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 1,
            actual: args.len(),
        });
    }
    if let Value::String(s) = &args[0] {
        unsafe {
            match libloading::Library::new(s) {
                Ok(lib) => Ok(Value::Library(Rc::new(lib))),
                Err(e) => Err(SelError::Runtime(
                    loc,
                    format!("dlopen failed: {}", e),
                )),
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
        return Err(SelError::ArityMismatch {
            loc: loc,
            expected: 2,
            actual: args.len(),
        });
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
            Err(e) => Err(SelError::Runtime(
                loc,
                format!("dlsym failed: {}", e),
            )),
        }
    }
}

pub fn ffi_call(loc: Loc, args: Vec<Value>) -> Result<Value> {

    let ptr = match args[0] {
        Value::Pointer(p) => p,
        _ => {
            return Err(SelError::Runtime(
                loc,
                "ffi-call requires a pointer".into(),
            ));
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
            for v in l {
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
            return Err(SelError::Runtime(
                loc,
                "arg_types must be a list".into(),
            ));
        }
    };

    let arg_vals = match args[3].clone() {
        Value::List(l) => l,
        Value::Nil => Vec::new(),
        _ => {
            return Err(SelError::Runtime(
                loc,
                "arg_vals must be a list".into(),
            ));
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
        let sym = &arg_type_syms[i];
        match sym.as_str() {
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
                    Ok(Value::String(c_str.to_string_lossy().into_owned()))
                }
            }
            _ => unreachable!(),
        }
    }
}

pub fn io_read_string(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if let Value::String(path) = &args[0] {
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(Value::String(content)),
            Err(e) => Err(SelError::Runtime(
                loc,
                format!("io/read-string failed: {}", e),
            )),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "io/read-string requires a string argument".into(),
        ))
    }
}

pub fn io_write_string(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if let (Value::String(path), Value::String(content)) = (&args[0], &args[1]) {
        match std::fs::write(path, content) {
            Ok(_) => Ok(Value::Nil),
            Err(e) => Err(SelError::Runtime(
                loc,
                format!("io/write-string failed: {}", e),
            )),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "io/write-string requires string arguments".into(),
        ))
    }
}

pub fn io_file_exists(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if let Value::String(path) = &args[0] {
        Ok(Value::Boolean(std::path::Path::new(path).exists()))
    } else {
        Err(SelError::Runtime(
            loc,
            "io/file-exists? requires a string argument".into(),
        ))
    }
}

pub fn os_getenv(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if let Value::String(key) = &args[0] {
        match std::env::var(key) {
            Ok(val) => Ok(Value::String(val)),
            Err(_) => Ok(Value::Nil),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "os/getenv requires a string argument".into(),
        ))
    }
}

pub fn os_args(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let args_vec = std::env::args()
        .skip(1)
        .map(Value::String)
        .collect::<Vec<_>>();
    Ok(Value::List(args_vec))
}

pub fn os_orig_args(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let args_vec = std::env::args().map(Value::String).collect::<Vec<_>>();
    Ok(Value::List(args_vec))
}

pub fn load(env: Rc<RefCell<Env>>) {

    // let mut e = env.borrow_mut();
    // e.insert(intern("+"), Value::NativeFunction(sum));
    // e.insert(intern("-"), Value::NativeFunction(sub));
    // e.insert(intern("*"), Value::NativeFunction(mul));
    // e.insert(intern("/"), Value::NativeFunction(div));
    // e.insert(intern("mod"), Value::NativeFunction(modulo));

    // e.insert(intern("eq?"), Value::NativeFunction(is_equal));
    // e.insert(intern("="), Value::NativeFunction(num_eq));
    // e.insert(intern("!="), Value::NativeFunction(num_noteq));
    // e.insert(intern("<"), Value::NativeFunction(num_lt));
    // e.insert(intern(">"), Value::NativeFunction(num_gt));
    // e.insert(intern("<="), Value::NativeFunction(num_lte));
    // e.insert(intern(">="), Value::NativeFunction(num_gte));
    // e.insert(intern("cons"), Value::NativeFunction(cons));
    // e.insert(intern("car"), Value::NativeFunction(car));
    // e.insert(intern("cdr"), Value::NativeFunction(cdr));
    // e.insert(intern("nth"), Value::NativeFunction(nth));
    // e.insert(intern("count"), Value::NativeFunction(count));
    // e.insert(intern("list"), Value::NativeFunction(list));
    // e.insert(intern("empty?"), Value::NativeFunction(empty));
    // e.insert(intern("nil?"), Value::NativeFunction(is_nil));
    // e.insert(intern("list?"), Value::NativeFunction(is_list));
    // e.insert(intern("number?"), Value::NativeFunction(is_number));
    // e.insert(intern("string?"), Value::NativeFunction(is_string));
    // e.insert(intern("symbol?"), Value::NativeFunction(is_symbol));
    // e.insert(intern("function?"), Value::NativeFunction(is_function));
    // e.insert(intern("type-of"), Value::NativeFunction(type_of));
    // e.insert(intern("error"), Value::NativeFunction(error));
    // e.insert(intern("not"), Value::NativeFunction(not));
    // e.insert(intern("display"), Value::NativeFunction(display));
    // e.insert(intern("println"), Value::NativeFunction(display_newline));
    // e.insert(intern("newline"), Value::NativeFunction(newline));
    // e.insert(intern("ffi-dlopen"), Value::NativeFunction(ffi_dlopen));
    // e.insert(intern("ffi-dlsym"), Value::NativeFunction(ffi_dlsym));
    // e.insert(intern("ffi-call"), Value::NativeFunction(ffi_call));

    // e.insert(
    // intern("io/read-string"),
    // Value::NativeFunction(io_read_string),
    // );
    // e.insert(
    // intern("io/write-string"),
    // Value::NativeFunction(io_write_string),
    // );
    // e.insert(
    // intern("io/file-exists?"),
    // Value::NativeFunction(io_file_exists),
    // );
    // e.insert(intern("os/getenv"), Value::NativeFunction(os_getenv));
    // e.insert(intern("os/args"), Value::NativeFunction(os_args));
    // e.insert(intern("os/orig-args"), Value::NativeFunction(os_orig_args));

}
