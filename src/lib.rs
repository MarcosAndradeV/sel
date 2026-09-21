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
pub use diagnostics::SelErrorKind;
pub use internal::load_core_lib;
pub use lexer::Loc;
pub use runtime::Env;
pub use types::{intern, lookup};
pub use value::Value;

use std::cell::RefCell;
use std::rc::Rc;

use std::path::PathBuf;

/// Evaluate a Scheme source string in the given environment and return the result.
pub fn eval(source: &str, env: Rc<RefCell<Env>>) -> std::result::Result<Value, SelError> {
    eval_sandboxed(source, env, None)
}

/// Evaluate a Scheme source string with an optional filesystem sandbox directory.
pub fn eval_sandboxed(
    source: &str,
    env: Rc<RefCell<Env>>,
    sandbox_root: Option<PathBuf>,
) -> std::result::Result<Value, SelError> {
    let mut diags = Vec::new();
    let file_id = types::intern("<embedded>");
    let asts = parser::parse_all(source, file_id, &mut diags);
    if !diags.is_empty() {
        return Err(diags.remove(0));
    }
    runtime::execute_asts_sandboxed(asts, env, sandbox_root)
}

/// Evaluate a Scheme file in the given environment and return the result.
pub fn load_file(script_path: &str, env: Rc<RefCell<Env>>) -> Result<Value, SelError> {
    load_file_sandboxed(script_path, env, None)
}

/// Evaluate a Scheme file with an optional filesystem sandbox directory.
pub fn load_file_sandboxed(
    script_path: &str,
    env: Rc<RefCell<Env>>,
    sandbox_root: Option<PathBuf>,
) -> Result<Value, SelError> {
    let target_path = PathBuf::from(script_path);
    if let Some(ref root) = sandbox_root {
        let canonical_path = target_path.canonicalize().map_err(|e| {
            SelError::SandboxViolation(Loc::default(), format!("Failed to resolve path {}: {}", target_path.display(), e))
        })?;
        let canonical_root = root.canonicalize().map_err(|e| {
            SelError::SandboxViolation(Loc::default(), format!("Failed to resolve sandbox root {}: {}", root.display(), e))
        })?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(SelError::SandboxViolation(
                Loc::default(),
                format!(
                    "Sandbox violation: path {} is outside root {}",
                    canonical_path.display(),
                    canonical_root.display()
                ),
            ));
        }
    }
    let src = internal::read_script(script_path)?;
    let mut diags = Vec::new();
    let file_id = intern(script_path);
    let asts = parser::parse_all(&src, file_id, &mut diags);
    if !diags.is_empty() {
        for diag in diags {
            eprintln!("{}", diag);
        }
        return Err(SelError::Trace("invalid syntax".into()));
    }
    runtime::execute_asts_sandboxed(asts, env.clone(), sandbox_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_co_yield_colosures() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        // Basic calculation
        let res = eval("(define f \\() (co-yield)) (f)", env.clone()).unwrap();
        assert!(matches!(res, Value::Nil));
    }

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

    #[test]
    fn test_sandboxed_eval() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        let current_dir = std::env::current_dir().unwrap();
        let tests_dir = current_dir.join("tests");

        // Loading within sandbox should succeed
        let script_path = tests_dir.join("helper_load.scm");
        let res = load_file_sandboxed(
            script_path.to_str().unwrap(),
            env.clone(),
            Some(tests_dir.clone()),
        );
        assert!(res.is_ok());

        // Loading from outside sandbox should fail with SandboxViolation
        let root_outside = tests_dir.join("errors"); // Sandbox root is tests/errors
        let res_denied = load_file_sandboxed(
            script_path.to_str().unwrap(),
            env,
            Some(root_outside),
        );
        assert!(res_denied.is_err());
        assert!(matches!(res_denied.unwrap_err(), SelError::SandboxViolation(_, _)));
    }

    #[test]
    fn test_error_improvements() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        // TypeError
        let res_type = eval("(+ \"hello\" 1)", env.clone());
        assert!(res_type.is_err());
        let err_type = res_type.unwrap_err();
        assert_eq!(err_type.kind(), SelErrorKind::Type);
        assert!(err_type.loc().is_some());
        assert!(err_type.message().contains("Invalid argument to +: expected number"));

        // Undefined NameError
        let res_name = eval("(non-existent-variable)", env);
        assert!(res_name.is_err());
        let err_name = res_name.unwrap_err();
        assert_eq!(err_name.kind(), SelErrorKind::Name);
        assert!(err_name.loc().is_some());
        assert_eq!(err_name.message(), "Undefined variable `non-existent-variable`");
    }

    #[test]
    fn test_slice_window_list_and_string() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        // Test list cdr and drop
        let res1 = eval("(cdr '(1 2 3))", env.clone()).unwrap();
        assert_eq!(format!("{res1}"), "(2 3)");

        let res2 = eval("(cdr (cdr '(1 2 3)))", env.clone()).unwrap();
        assert_eq!(format!("{res2}"), "(3)");

        let res3 = eval("(cdr (cdr (cdr '(1 2 3))))", env.clone()).unwrap();
        assert_eq!(format!("{res3}"), "()");

        let res4 = eval("(drop 2 '(10 20 30 40))", env.clone()).unwrap();
        assert_eq!(format!("{res4}"), "(30 40)");

        let res5 = eval("(nth (cdr '(10 20 30 40)) 1)", env.clone()).unwrap();
        assert!(matches!(res5, Value::Integer(30)));

        // Test string cdr and drop
        let s1 = eval("(cdr \"hello\")", env.clone()).unwrap();
        assert_eq!(format!("{s1}"), "ello");

        let s2 = eval("(cdr (cdr \"hello\"))", env.clone()).unwrap();
        assert_eq!(format!("{s2}"), "llo");

        let s3 = eval("(drop 3 \"abcdef\")", env.clone()).unwrap();
        assert_eq!(format!("{s3}"), "def");

        let s4 = eval("(car (cdr \"world\"))", env.clone()).unwrap();
        assert!(matches!(s4, Value::Char('o')));

        let s5 = eval("(nth (cdr \"world\") 2)", env.clone()).unwrap();
        assert!(matches!(s5, Value::Char('l')));

        // Test list equality with different offsets
        let eq_test = eval("(eq? (cdr '(1 2 3)) '(2 3))", env.clone()).unwrap();
        assert!(matches!(eq_test, Value::Boolean(true)));

        // Test string equality with different offsets
        let s_eq_test = eval("(eq? (cdr \"abc\") \"bc\")", env).unwrap();
        assert!(matches!(s_eq_test, Value::Boolean(true)));
    }
}

