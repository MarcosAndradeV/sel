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
pub struct Closure {
    pub params: Vec<u32>,
    pub chunk: Rc<Chunk>,
    pub env: Rc<RefCell<Env>>,
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub params: Vec<u32>,
    pub chunk: Rc<Chunk>,
    pub env: Rc<RefCell<Env>>,
}

#[derive(Clone)]
pub struct NativeClosureFn(pub Rc<dyn Fn(Loc, Vec<Value>) -> Result<Value>>);

impl std::fmt::Debug for NativeClosureFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native-closure>")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineState {
    Suspended,
    Running,
    Dead,
}

pub struct Coroutine {
    pub state: std::cell::Cell<CoroutineState>,
    pub frames: RefCell<Vec<crate::runtime::CallFrame>>,
    pub operand_stack: RefCell<Vec<Value>>,
    pub closure: Rc<Closure>,
}

impl std::fmt::Debug for Coroutine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<coroutine>")
    }
}

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
    Closure(Rc<Closure>),
    NativeFunction(fn(loc: Loc, args: Vec<Value>) -> Result<Value>),
    #[allow(unused)]
    NativeClosure(NativeClosureFn),
    Macro(Rc<Macro>),
    Pointer(usize),
    Library(Rc<libloading::Library>),
    Coroutine(Rc<Coroutine>),
    Char(char),
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
        Value::Char(c) => match c {
            ' ' => "#\\space".to_string(),
            '\n' => "#\\newline".to_string(),
            '\t' => "#\\tab".to_string(),
            '\r' => "#\\return".to_string(),
            ch => format!("#\\{}", ch),
        },
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
        Value::Closure(_) => "<closure>".to_string(),
        Value::NativeClosure(_) => "<native-closure>".to_string(),
        Value::NativeFunction { .. } => "<function>".to_string(),
        Value::Macro(_) => "<macro>".to_string(),
        Value::Pointer(p) => format!("<pointer: {:#x}>", p),
        Value::Library(_) => "<library>".to_string(),
        Value::Coroutine(_) => "<coroutine>".to_string(),
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_value(self))
    }
}
