use std::cell::RefCell;
use std::rc::Rc;

use sel::diagnostics::SelError;
use sel::eval;
use sel::internal::load_core_lib;
use sel::runtime::Env;
use sel::value::Value;

fn test_env() -> Rc<RefCell<Env>> {
    let env = Rc::new(RefCell::new(Env::default()));
    env.borrow_mut().parent = Some(load_core_lib());
    env
}

#[test]
fn test_runtime_nominal_constructor_and_transparent_access() {
    let env = test_env();
    let src = r#"
newtype Foo := { c: int }
f := Foo({ c: 42 })
f.c
"#;
    let res = eval(src, env).expect("eval should succeed");
    assert!(matches!(res, Value::Integer(42)));
}

#[test]
fn test_runtime_nominal_symbol_access_and_rset() {
    let env = test_env();
    let src = r#"
newtype Foo := { c: int }
f := Foo({ c: 42 })
updated := rset(f, :c, 99)
updated.c
"#;
    let res = eval(src, env).expect("eval should succeed");
    assert!(matches!(res, Value::Integer(99)));
}

#[test]
fn test_runtime_nominal_type_of() {
    let env = test_env();
    let src = r#"
newtype Bar := { name: string }
b := Bar({ name: "sel" })
type_of(b)
"#;
    let res = eval(src, env).expect("eval should succeed");
    if let Value::Symbol(sym) = res {
        assert_eq!(sel::types::lookup(sym), "Bar");
    } else {
        panic!("Expected symbol Bar, got {:?}", res);
    }
}

#[test]
fn test_runtime_nominal_equality() {
    let env = test_env();
    let src = r#"
newtype Foo := { c: int }
f1 := Foo({ c: 10 })
f2 := Foo({ c: 10 })
f3 := Foo({ c: 20 })
raw := { c: 10 }

[f1 == f2, f1 == f3, f1 == raw]
"#;
    let res = eval(src, env).expect("eval should succeed");
    if let Value::List(vec) = res {
        assert_eq!(vec.len(), 3);
        assert!(matches!(vec[0], Value::Boolean(true)));
        assert!(matches!(vec[1], Value::Boolean(false)));
        assert!(matches!(vec[2], Value::Boolean(false))); // nominal != structural
    } else {
        panic!("Expected list, got {:?}", res);
    }
}

#[test]
fn test_runtime_static_type_gate_prevents_execution() {
    let env = test_env();
    let src = r#"
add :: int, int -> int
add x y := x + y

// Compile-time type mismatch: string passed where int expected
add("hello", 42)
"#;
    let res = eval(src, env);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("Type mismatch in argument 1"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_runtime_static_gate_rejects_missing_pub_signature() {
    let env = test_env();
    let src = r#"
add :: int, int -> int
add x y := x + y

pub unannotated_export x := x + 1
"#;
    let res = eval(src, env);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("requires an explicit type signature"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_runtime_showable_custom_implementation() {
    let env = test_env();
    let src = r#"
newtype Foo := { c: int }
implements(Foo, :showable, \self -> format("CustomFoo({})", self.c))

f := Foo({ c: 42 })
to_string(f)
"#;
    let res = eval(src, env).expect("eval should succeed");
    assert_eq!(res.to_string_lossy(), Some("CustomFoo(42)".to_string()));
}

#[test]
fn test_runtime_comparable_custom_implementation() {
    let env = test_env();
    let src = r#"
newtype EvenOdd := { n: int }
// Custom comparable: equal if both have same parity
implements(EvenOdd, :comparable, \a b -> (a.n % 2) == (b.n % 2))

e1 := EvenOdd({ n: 2 })
e2 := EvenOdd({ n: 4 })
o1 := EvenOdd({ n: 3 })

[e1 == e2, e1 == o1]
"#;
    let res = eval(src, env).expect("eval should succeed");
    if let Value::List(vec) = res {
        assert_eq!(vec.len(), 2);
        assert!(matches!(vec[0], Value::Boolean(true)));
        assert!(matches!(vec[1], Value::Boolean(false)));
    } else {
        panic!("Expected list, got {:?}", res);
    }
}

#[test]
fn test_modern_value_formatting() {
    let env = test_env();
    let src = r#"
[nil, true, false, [1, 2, 3], { c: 10 }]
"#;
    let res = eval(src, env).expect("eval should succeed");
    assert_eq!(format!("{res}"), "[nil, true, false, [1, 2, 3], { c: 10 }]");
}

