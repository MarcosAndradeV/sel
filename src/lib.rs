pub mod ast;
pub mod compiler;
pub mod diagnostics;
pub mod internal;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod types;
pub mod value;

// Re-exports
pub use diagnostics::SelError;
pub use internal::load_core_lib;
pub use lexer::Loc;
pub use runtime::Env;
pub use types::{intern, lookup};
pub use value::Value;

use std::cell::RefCell;
use std::rc::Rc;

/// Evaluate a Scheme source string in the given environment and return the result.
pub fn eval(source: &str, env: Rc<RefCell<Env>>) -> std::result::Result<Value, SelError> {
    let mut diags = Vec::new();
    let file_id = types::intern("<embedded>");
    let asts = parser::parse_all(source, file_id, &mut diags);
    if !diags.is_empty() {
        return Err(diags.remove(0));
    }
    runtime::execute_asts(asts, env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_eval() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        // Basic calculation
        let res = eval("(+ 1 2 3)", env.clone()).unwrap();
        assert!(matches!(res, Value::Integer(6)));

        // Variable binding and lookup
        eval("(define my-var 100)", env.clone()).unwrap();
        let res2 = eval("(* my-var 2)", env.clone()).unwrap();
        assert!(matches!(res2, Value::Integer(200)));

        // Injecting a custom Rust function
        fn custom_sum(_loc: Loc, args: Vec<Value>) -> std::result::Result<Value, SelError> {
            let mut sum = 0;
            for arg in args {
                if let Value::Integer(i) = arg {
                    sum += i;
                }
            }
            Ok(Value::Integer(sum))
        }

        env.borrow_mut()
            .insert(intern("custom-sum"), Value::NativeFunction(custom_sum));

        let res3 = eval("(custom-sum 10 20 30)", env).unwrap();
        assert!(matches!(res3, Value::Integer(60)));
    }
}
