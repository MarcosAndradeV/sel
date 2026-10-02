use sel::diagnostics::SelError;
use sel::parser::parse_all;
use sel::typecheck::typecheck_program;
use sel::types::intern;

fn check_code(src: &str) -> Result<(), SelError> {
    let mut diags = Vec::new();
    let asts = parse_all(src, intern("<test>"), &mut diags);
    assert!(diags.is_empty(), "parse errors: {:?}", diags);
    typecheck_program(&asts)
}

#[test]
fn test_concrete_signature_success() {
    let src = r#"
add :: int, int -> int
add x y := 42
res := add(10, 20)
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_concrete_signature_arg_mismatch_fails() {
    let src = r#"
add :: int, int -> int
add x y := 42
res := add("not an int", 20)
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("Type mismatch in argument 1"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_mandatory_pub_signature_fails_when_missing() {
    let src = r#"
add :: int, int -> int
add x y := 42

pub unannotated_pub x := x + 1
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("Public function `unannotated_pub` requires an explicit type signature"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_mandatory_pub_signature_succeeds_when_annotated() {
    let src = r#"
pub greet :: string -> string
pub greet name := name
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_generic_function_instantiation() {
    let src = r#"
id :: forany(A) : A -> A
id x := x

a := id(10)
b := id("hello")
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_trait_bounds_satisfaction() {
    let src = r#"
check_eq :: forany(A) where implements(A, :comparable) : A, A -> bool
check_eq a b := true

x := check_eq(10, 20)
y := check_eq("a", "b")
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_trait_bounds_failure_on_non_implementing_type() {
    let src = r#"
check_eq :: forany(A) where implements(A, :comparable) : A, A -> bool
check_eq a b := true

newtype Uncomparable := { data: int }
val := check_eq(Uncomparable({ data: 1 }), Uncomparable({ data: 2 }))
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("does not implement required trait `:comparable`"));
    } else {
        panic!("Expected trait failure TypeError, got {:?}", res);
    }
}

#[test]
fn test_nominal_constructor_and_transparent_dot_access() {
    let src = r#"
newtype Foo := { c: int }

process :: Foo -> int
process f := f.c

// 1. Passing constructor Foo(...) succeeds
res1 := process(Foo({ c: 10 }))

// 2. Direct field access f.c on nominal type works
f := Foo({ c: 42 })
val := f.c
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_nominal_type_rejects_raw_structural_record() {
    let src = r#"
newtype Foo := { c: int }

process :: Foo -> int
process f := f.c

// Raw structural record must be rejected when Foo is expected!
res := process({ c: 10 })
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("Type mismatch"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_derived_trait_allows_trait_bounded_call() {
    let src = r#"
newtype Foo := { c: int }
derive(Foo, :comparable)

check_eq :: forany(A) where implements(A, :comparable) : A, A -> bool
check_eq a b := true

// Foo now derives :comparable, so this must pass!
res := check_eq(Foo({ c: 1 }), Foo({ c: 2 }))
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_inline_type_assertion() {
    let src = r#"
x := { c: 10 } :: record
y := 42 :: int
"#;
    assert!(check_code(src).is_ok());

    let bad_src = r#"
x := "string" :: int
"#;
    let res = check_code(bad_src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("Type assertion failed"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_user_trait_definition_and_valid_implementation() {
    let src = r#"
trait :geometry := {
    area: Self -> float,
    perimeter: Self -> float
}

newtype Circle := { radius: float }

implements(Circle, :geometry, {
    area: \c -> 3.14159 * c.radius * c.radius,
    perimeter: \c -> 2.0 * 3.14159 * c.radius
})

c := Circle({ radius: 10.0 })
res := area(c)
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_user_trait_missing_method_fails() {
    let src = r#"
trait :geometry := {
    area: Self -> float,
    perimeter: Self -> float
}

newtype Circle := { radius: float }

implements(Circle, :geometry, {
    area: \c -> 3.14159 * c.radius * c.radius
})
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("missing required method `perimeter`"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_user_trait_extra_method_fails() {
    let src = r#"
trait :geometry := {
    area: Self -> float
}

newtype Circle := { radius: float }

implements(Circle, :geometry, {
    area: \c -> 3.14159 * c.radius * c.radius,
    volume: \c -> 0.0
})
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("not declared in trait :geometry"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_user_trait_non_implementing_type_call_fails() {
    let src = r#"
trait :geometry := {
    area: Self -> float
}

newtype Circle := { radius: float }

implements(Circle, :geometry, {
    area: \c -> 3.14159 * c.radius * c.radius
})

newtype Person := { name: string }

res := area(Person({ name: "Alice" }))
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("does not implement required trait `:geometry`"), "actual msg: {}", msg);
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}

#[test]
fn test_user_trait_single_method_shorthand_success() {
    let src = r#"
trait :printable := {
    to_string: Self -> string
}

newtype User := { name: string }

implements(User, :printable, \u -> u.name)

u := User({ name: "Alice" })
res := to_string(u)
"#;
    assert!(check_code(src).is_ok());
}

#[test]
fn test_user_trait_single_method_shorthand_on_multi_method_trait_fails() {
    let src = r#"
trait :geometry := {
    area: Self -> float,
    perimeter: Self -> float
}

newtype Circle := { radius: float }

implements(Circle, :geometry, \c -> 1.0)
"#;
    let res = check_code(src);
    assert!(res.is_err());
    if let Err(SelError::TypeError(_, msg)) = res {
        assert!(msg.contains("must provide a record of methods"));
    } else {
        panic!("Expected TypeError, got {:?}", res);
    }
}
