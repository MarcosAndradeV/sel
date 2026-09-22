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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringSlice {
    pub text: Rc<str>,
    pub start: usize,
    pub end: usize,
}

impl StringSlice {
    #[inline]
    pub fn new(s: &str) -> Self {
        let text: Rc<str> = s.into();
        let end = text.len();
        Self {
            text,
            start: 0,
            end,
        }
    }

    #[inline]
    pub fn from_rc(text: Rc<str>, start: usize, end: usize) -> Self {
        Self { text, start, end }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.text[self.start..self.end]
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    #[inline]
    pub fn char_count(&self) -> usize {
        self.as_str().chars().count()
    }

    #[inline]
    pub fn slice_byte_offset(&self, byte_offset: usize) -> Self {
        let new_start = (self.start + byte_offset).min(self.end);
        Self {
            text: self.text.clone(),
            start: new_start,
            end: self.end,
        }
    }

    pub fn drop_chars(&self, n: usize) -> Self {
        if n == 0 {
            return self.clone();
        }
        for (count, (idx, _)) in self.as_str().char_indices().enumerate() {
            if count == n {
                return self.slice_byte_offset(idx);
            }
        }
        self.slice_byte_offset(self.len())
    }
}

impl std::fmt::Display for StringSlice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Symbol(u32),
    String(Box<StringSlice>),
    List(Box<imbl::Vector<Self>>),
    Record(Rc<Record<Self>>),
    Closure(Rc<Closure>),
    NativeFunction(fn(loc: Loc, args: Vec<Value>) -> Result<Value>),
    #[allow(unused)]
    NativeClosure(Rc<NativeClosureFn>),
    Macro(Rc<Macro>),
    Pointer(usize),
    #[cfg(feature = "ffi")]
    Library(Rc<libloading::Library>),
    Coroutine(Rc<Coroutine>),
    Char(char),
}

impl Value {
    #[inline]
    pub fn make_list(items: Vec<Value>) -> Self {
        Value::List(Box::new(items.into()))
    }

    #[inline]
    pub fn make_string(s: &str) -> Self {
        Value::String(Box::new(StringSlice::new(s)))
    }

    #[inline]
    pub fn to_string_lossy(&self) -> Option<String> {
        match self {
            Value::String(s) => Some(s.as_str().to_string()),
            Value::List(l) => {
                let mut s = String::with_capacity(l.len());
                for v in l.iter() {
                    if let Value::Char(c) = v {
                        s.push(*c);
                    } else {
                        return None;
                    }
                }
                Some(s)
            }
            _ => None,
        }
    }

    #[inline]
    pub fn as_str_lossy(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_char(&self) -> Option<&char> {
        if let Self::Char(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn is_char(&self) -> bool {
        matches!(self, Value::Char(_))
    }
}

fn format_value(val: &Value) -> String {
    match val {
        Value::Nil => "()".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
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
        Value::String(s) => s.as_str().to_string(),
        Value::List(l) => {
            if !l.is_empty() && l.iter().all(|v| matches!(v, Value::Char(_))) {
                l.iter()
                    .map(|v| match v {
                        Value::Char(c) => *c,
                        _ => unreachable!(),
                    })
                    .collect()
            } else {
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
        #[cfg(feature = "ffi")]
        Value::Library(_) => "<library>".to_string(),
        Value::Coroutine(_) => "<coroutine>".to_string(),
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_value(self))
    }
}
