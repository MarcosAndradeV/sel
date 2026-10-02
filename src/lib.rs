pub mod analysis;
pub mod ast;
pub mod compiler;
pub mod debugger;
pub mod diagnostics;
pub mod internal;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod typecheck;
pub mod types;
pub mod value;

// Re-exports
pub use debugger::{DebugSession, DisassembledInstruction, VmSnapshot, VmStatus};
pub use diagnostics::{SelError, SelErrorKind, SelWarning, StackFrame};
pub use internal::load_core_lib;
pub use lexer::Loc;
pub use runtime::Env;
pub use types::{intern, lookup};
pub use value::Value;

use std::cell::RefCell;
use std::rc::Rc;

use std::path::PathBuf;

/// Evaluate a SEL source string in the given environment and return the result.
pub fn eval(source: &str, env: Rc<RefCell<Env>>) -> std::result::Result<Value, SelError> {
    eval_sandboxed(source, env, None)
}

/// Evaluate a SEL source string with an optional filesystem sandbox directory.
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

/// Evaluate a SEL file in the given environment and return the result.
pub fn load_file(script_path: &str, env: Rc<RefCell<Env>>) -> Result<Value, SelError> {
    load_file_sandboxed(script_path, env, None)
}

/// Evaluate a SEL file with an optional filesystem sandbox directory.
pub fn load_file_sandboxed(
    script_path: &str,
    env: Rc<RefCell<Env>>,
    sandbox_root: Option<PathBuf>,
) -> Result<Value, SelError> {
    let target_path = PathBuf::from(script_path);
    if let Some(ref root) = sandbox_root {
        let canonical_path = target_path.canonicalize().map_err(|e| {
            SelError::SandboxViolation(
                Loc::default(),
                format!("Failed to resolve path {}: {}", target_path.display(), e),
            )
        })?;
        let canonical_root = root.canonicalize().map_err(|e| {
            SelError::SandboxViolation(
                Loc::default(),
                format!("Failed to resolve sandbox root {}: {}", root.display(), e),
            )
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
    use crate::value::Arity;

use super::*;

    #[test]
    fn test_co_yield_colosures() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        let res = eval("f := \\ -> yield nil\nf()", env.clone()).unwrap();
        assert!(matches!(res, Value::Nil));
    }

    #[test]
    fn test_library_eval() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        // Basic calculation
        let res = eval("1 + 2 + 3", env.clone()).unwrap();
        assert!(matches!(res, Value::Integer(6)));

        // Variable binding and lookup
        eval("my_var := 100", env.clone()).unwrap();
        let res2 = eval("my_var * 2", env.clone()).unwrap();
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
            .insert(intern("custom_sum"), Value::NativeFunction(custom_sum, Arity::Variadic));

        let res3 = eval("custom_sum(10, 20, 30)", env).unwrap();
        assert!(matches!(res3, Value::Integer(60)));
    }

    #[test]
    fn test_sandboxed_eval() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        let current_dir = std::env::current_dir().unwrap();
        let tests_dir = current_dir.join("tests");

        // Loading within sandbox should succeed
        let script_path = tests_dir.join("helper_load.sel");
        let res = load_file_sandboxed(
            script_path.to_str().unwrap(),
            env.clone(),
            Some(tests_dir.clone()),
        );
        assert!(res.is_ok());

        // Loading from outside sandbox should fail with SandboxViolation
        let root_outside = tests_dir.join("errors"); // Sandbox root is tests/errors
        let res_denied =
            load_file_sandboxed(script_path.to_str().unwrap(), env, Some(root_outside));
        assert!(res_denied.is_err());
        assert!(matches!(
            res_denied.unwrap_err(),
            SelError::SandboxViolation(_, _)
        ));
    }

    #[test]
    fn test_error_improvements() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        // TypeError
        let res_type = eval("\"hello\" + 1", env.clone());
        assert!(res_type.is_err());
        let err_type = res_type.unwrap_err();
        assert_eq!(err_type.kind(), SelErrorKind::Type);
        assert!(err_type.loc().is_some());
        assert!(
            err_type
                .message()
                .contains("Invalid argument to +: expected number")
        );

        // Undefined NameError
        let res_name = eval("non_existent_variable", env);
        assert!(res_name.is_err());
        let err_name = res_name.unwrap_err();
        assert_eq!(err_name.kind(), SelErrorKind::Name);
        assert!(err_name.loc().is_some());
        assert_eq!(
            err_name.message(),
            "Undefined variable `non_existent_variable`"
        );
    }

    #[test]
    fn test_stack_trace() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        let code = "inc i := i + 1\ninc(65)\ninc(\"Hello\")\n";
        let res = eval(code, env.clone());
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), SelErrorKind::Type);
        let bt = err.backtrace().expect("expected backtrace");
        assert_eq!(bt.len(), 2);
        assert_eq!(bt[0].function_name.as_deref(), Some("inc"));
        assert_eq!(bt[1].function_name.as_deref(), Some("<main>"));

        let err_str = err.to_string();
        assert!(err_str.contains("stack backtrace:"));
        assert!(err_str.contains("0: inc at <embedded>:1:12"));
        assert!(err_str.contains("1: <main> at <embedded>:3:4"));

        // Multi-level call stack
        let code_multi = "inc i := i + 1\nf x := inc(x) + 0\ng y := f(y) + 0\ng(\"Hello\")\n";
        let res_multi = eval(code_multi, env.clone());
        let err_multi = res_multi.unwrap_err();
        let bt_multi = err_multi.backtrace().expect("expected backtrace");
        assert_eq!(bt_multi.len(), 4);
        assert_eq!(bt_multi[0].function_name.as_deref(), Some("inc"));
        assert_eq!(bt_multi[1].function_name.as_deref(), Some("f"));
        assert_eq!(bt_multi[2].function_name.as_deref(), Some("g"));
        assert_eq!(bt_multi[3].function_name.as_deref(), Some("<main>"));

        // Anonymous lambda
        let code_anon = "(\\x -> x + 1)(\"Hello\")\n";
        let res_anon = eval(code_anon, env.clone());
        let err_anon = res_anon.unwrap_err();
        let bt_anon = err_anon.backtrace().expect("expected backtrace");
        assert_eq!(bt_anon[0].function_name.as_deref(), None);

        // Top-level failure with no function calls: backtrace has 1 frame and display doesn't show stack backtrace header
        let code_top = "1 + \"bad\"";
        let res_top = eval(code_top, env);
        let err_top = res_top.unwrap_err();
        let bt_top = err_top.backtrace().expect("expected backtrace");
        assert_eq!(bt_top.len(), 1);
        assert!(!err_top.to_string().contains("stack backtrace:"));
    }

    #[test]
    fn test_slice_window_list_and_string() {
        let env = Rc::new(RefCell::new(Env::default()));
        env.borrow_mut().parent = Some(load_core_lib());

        // Test list rest and drop
        let res1 = eval("rest([1, 2, 3])", env.clone()).unwrap();
        assert_eq!(format!("{res1}"), "[2, 3]");

        let res2 = eval("rest(rest([1, 2, 3]))", env.clone()).unwrap();
        assert_eq!(format!("{res2}"), "[3]");

        let res3 = eval("rest(rest(rest([1, 2, 3])))", env.clone()).unwrap();
        assert_eq!(format!("{res3}"), "nil");

        let res4 = eval("drop(2, [10, 20, 30, 40])", env.clone()).unwrap();
        assert_eq!(format!("{res4}"), "[30, 40]");

        let res5 = eval("nth(rest([10, 20, 30, 40]), 1)", env.clone()).unwrap();
        assert!(matches!(res5, Value::Integer(30)));

        // Test string rest and drop
        let s1 = eval("rest(\"hello\")", env.clone()).unwrap();
        assert_eq!(format!("{s1}"), "ello");

        let s2 = eval("rest(rest(\"hello\"))", env.clone()).unwrap();
        assert_eq!(format!("{s2}"), "llo");

        let s3 = eval("drop(3, \"abcdef\")", env.clone()).unwrap();
        assert_eq!(format!("{s3}"), "def");

        let s4 = eval("first(rest(\"world\"))", env.clone()).unwrap();
        assert!(matches!(s4, Value::Char('o')));

        let s5 = eval("nth(rest(\"world\"), 2)", env.clone()).unwrap();
        assert!(matches!(s5, Value::Char('l')));

        // Test list equality with different offsets
        let eq_test = eval("rest([1, 2, 3]) == [2, 3]", env.clone()).unwrap();
        assert!(matches!(eq_test, Value::Boolean(true)));

        // Test string equality with different offsets
        let s_eq_test = eval("rest(\"abc\") == \"bc\"", env.clone()).unwrap();
        assert!(matches!(s_eq_test, Value::Boolean(true)));

        // Test strings are lists and format properly
        let str_is_list = eval("is_list(\"hello\")", env.clone()).unwrap();
        assert!(matches!(str_is_list, Value::Boolean(true)));

        let cons_str = eval("cons('h', \"ello\")", env).unwrap();
        assert_eq!(format!("{cons_str}"), "hello");
    }
}
