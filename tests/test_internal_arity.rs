use sel::diagnostics::SelError;
use sel::internal::load_core_lib;
use sel::types::intern;
use sel::value::{Arity, Value};

#[test]
fn test_native_function_arity_variants() {
    let env = load_core_lib();
    let e = env.borrow();

    // Fixed arities
    if let Some(Value::NativeFunction(_, Arity::Arity(n))) = e.get(intern("cons")) {
        assert_eq!(n, 2);
    } else {
        panic!("expected NativeFunction with Arity(2) for cons");
    }

    if let Some(Value::NativeFunction(_, Arity::Arity(n))) = e.get(intern("car")) {
        assert_eq!(n, 1);
    } else {
        panic!("expected NativeFunction with Arity(1) for car");
    }

    if let Some(Value::NativeFunction(_, Arity::Arity(n))) = e.get(intern("rset")) {
        assert_eq!(n, 3);
    } else {
        panic!("expected NativeFunction with Arity(3) for rset");
    }

    if let Some(Value::NativeFunction(_, Arity::Arity(n))) = e.get(intern("newline")) {
        assert_eq!(n, 0);
    } else {
        panic!("expected NativeFunction with Arity(0) for newline");
    }

    // Variadic
    if let Some(Value::NativeFunction(_, Arity::Variadic)) = e.get(intern("+")) {
        // ok
    } else {
        panic!("expected NativeFunction with Variadic for +");
    }

    if let Some(Value::NativeFunction(_, Arity::Variadic)) = e.get(intern("list")) {
        // ok
    } else {
        panic!("expected NativeFunction with Variadic for list");
    }
}

#[test]
fn test_internal_functions_arity_mismatch_errors() {
    use sel::lexer::Loc;
    let loc = Loc::default();

    // cons expects 2
    match sel::internal::cons(loc, vec![Value::Integer(1)]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for cons, got {:?}", other),
    }

    // car expects 1
    match sel::internal::car(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for car, got {:?}", other),
    }

    // cdr expects 1
    match sel::internal::cdr(loc, vec![Value::Integer(1), Value::Integer(2)]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 2);
        }
        other => panic!("expected ArityMismatch for cdr, got {:?}", other),
    }

    // nth expects 2
    match sel::internal::nth(loc, vec![Value::Integer(1)]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for nth, got {:?}", other),
    }

    // drop expects 2
    match sel::internal::drop(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for drop, got {:?}", other),
    }

    // take expects 2
    match sel::internal::take(loc, vec![Value::Integer(1)]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for take, got {:?}", other),
    }

    // count expects 1
    match sel::internal::count(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for count, got {:?}", other),
    }

    // empty expects 1
    match sel::internal::empty(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for empty, got {:?}", other),
    }

    // rget expects 2
    match sel::internal::rget(loc, vec![Value::Nil]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for rget, got {:?}", other),
    }

    // rset expects 3
    match sel::internal::rset(loc, vec![Value::Nil, Value::Nil]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 3);
            assert_eq!(actual, 2);
        }
        other => panic!("expected ArityMismatch for rset, got {:?}", other),
    }

    // rdel expects 2
    match sel::internal::rdel(loc, vec![Value::Nil]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for rdel, got {:?}", other),
    }

    // rkeys expects 1
    match sel::internal::rkeys(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for rkeys, got {:?}", other),
    }

    // rvals expects 1
    match sel::internal::rvals(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for rvals, got {:?}", other),
    }

    // rcontains expects 2
    match sel::internal::rcontains(loc, vec![Value::Nil]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for rcontains, got {:?}", other),
    }

    // is_nil expects 1
    match sel::internal::is_nil(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for is_nil, got {:?}", other),
    }

    // is_list expects 1
    match sel::internal::is_list(loc, vec![Value::Nil, Value::Nil]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 2);
        }
        other => panic!("expected ArityMismatch for is_list, got {:?}", other),
    }

    // is_number expects 1
    match sel::internal::is_number(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for is_number, got {:?}", other),
    }

    // is_string expects 1
    match sel::internal::is_string(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for is_string, got {:?}", other),
    }

    // string_contains expects 2
    match sel::internal::string_contains(loc, vec![Value::Nil]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for string_contains, got {:?}", other),
    }

    // is_symbol expects 1
    match sel::internal::is_symbol(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for is_symbol, got {:?}", other),
    }

    // is_record expects 1
    match sel::internal::is_record(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for is_record, got {:?}", other),
    }

    // is_function expects 1
    match sel::internal::is_function(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for is_function, got {:?}", other),
    }

    // is_char expects 1
    match sel::internal::is_char(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for is_char, got {:?}", other),
    }

    // char_to_integer expects 1
    match sel::internal::char_to_integer(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for char_to_integer, got {:?}", other),
    }

    // integer_to_char expects 1
    match sel::internal::integer_to_char(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for integer_to_char, got {:?}", other),
    }

    // type_of expects 1
    match sel::internal::type_of(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for type_of, got {:?}", other),
    }

    // newline expects 0
    match sel::internal::newline(loc, vec![Value::Integer(1)]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 0);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for newline, got {:?}", other),
    }

    // not expects 1
    match sel::internal::not(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for not, got {:?}", other),
    }

    // modulo expects 2
    match sel::internal::modulo(loc, vec![Value::Integer(1)]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ArityMismatch for modulo, got {:?}", other),
    }

    // co_create expects 1
    match sel::internal::co_create(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for co_create, got {:?}", other),
    }

    // co_state expects 1
    match sel::internal::co_state(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for co_state, got {:?}", other),
    }

    // co_dead_p expects 1
    match sel::internal::co_dead_p(loc, vec![]) {
        Err(SelError::ArityMismatch { expected, actual, .. }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected ArityMismatch for co_dead_p, got {:?}", other),
    }
}

#[test]
fn test_take_and_drop_edge_cases() {
    use sel::lexer::Loc;
    let loc = Loc::default();

    let eq = |a: Value, b: Value| {
        matches!(
            sel::internal::is_equal(loc, vec![a, b]),
            Ok(Value::Boolean(true))
        )
    };

    // take([], 1) should be Nil, not panic
    let empty_list = Value::make_list(vec![]);
    let res = sel::internal::take(loc, vec![empty_list.clone(), Value::Integer(1)]).unwrap();
    assert!(eq(res, Value::Nil));

    // take([], 0)
    let res = sel::internal::take(loc, vec![empty_list.clone(), Value::Integer(0)]).unwrap();
    assert!(eq(res, Value::Nil));

    // take([1, 2], 5) should return [1, 2], not panic
    let list12 = Value::make_list(vec![Value::Integer(1), Value::Integer(2)]);
    let res = sel::internal::take(loc, vec![list12.clone(), Value::Integer(5)]).unwrap();
    assert!(eq(res, list12.clone()));

    // take([1, 2], 1) -> [1]
    let res = sel::internal::take(loc, vec![list12.clone(), Value::Integer(1)]).unwrap();
    assert!(eq(res, Value::make_list(vec![Value::Integer(1)])));

    // take([1, 2], 0) -> Nil
    let res = sel::internal::take(loc, vec![list12.clone(), Value::Integer(0)]).unwrap();
    assert!(eq(res, Value::Nil));

    // take string edge cases
    let s = Value::make_string("hello");
    let res = sel::internal::take(loc, vec![s.clone(), Value::Integer(10)]).unwrap();
    assert!(eq(res, s.clone()));

    let res = sel::internal::take(loc, vec![s.clone(), Value::Integer(0)]).unwrap();
    assert!(eq(res, Value::Nil));

    let empty_str = Value::make_string("");
    let res = sel::internal::take(loc, vec![empty_str.clone(), Value::Integer(2)]).unwrap();
    assert!(eq(res, Value::Nil));

    // drop edge cases
    let res = sel::internal::drop(loc, vec![empty_list.clone(), Value::Integer(1)]).unwrap();
    assert!(eq(res, Value::Nil));

    let res = sel::internal::drop(loc, vec![list12.clone(), Value::Integer(5)]).unwrap();
    assert!(eq(res, Value::Nil));

    let res = sel::internal::drop(loc, vec![list12.clone(), Value::Integer(1)]).unwrap();
    assert!(eq(res, Value::make_list(vec![Value::Integer(2)])));
}

