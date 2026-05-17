use std::cell::RefCell;
use std::rc::Rc;

use crate::compiler::Chunk;
use crate::diagnostics::SelError;
use crate::lexer::Loc;
use crate::runtime::Env;
use crate::types::Record;
use crate::types::lookup;

type Result<T> = std::result::Result<T, SelError>;

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Integer(i64),
    Float(f64),
    String(Rc<String>),
    Boolean(bool),
    Symbol(u32),
    List(Rc<Vec<Value>>),
    Record(Rc<Record<Self>>),
    Closure {
        params: Vec<u32>,
        chunk: Rc<Chunk>,
        env: Rc<RefCell<Env>>,
    },
    NativeFunction(fn(loc: Loc, args: Vec<Value>) -> Result<Value>),
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
        Value::Record(r) => {
            let mut s = String::from("{");
            for (i, (k, v)) in r.fields().iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(&lookup(*k));
                s.push(' ');
                s.push_str(&format_value(v));
            }
            s.push('}');
            s
        }
        Value::Closure { .. } => "<closure>".to_string(),
        Value::NativeFunction { .. } => "<function>".to_string(),
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
