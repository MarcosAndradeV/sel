use sel::ast::{Ast, BaseTypeKind, TypeExpr};
use sel::parser::parse_all;
use sel::types::{intern, lookup};

#[test]
fn test_parse_simple_signature() {
    let mut diags = Vec::new();
    let src = "add :: int, int -> int";
    let asts = parse_all(src, intern("<test>"), &mut diags);
    assert!(diags.is_empty(), "diags: {:?}", diags);
    assert_eq!(asts.len(), 1);

    if let Ast::TypeSignature(_, name, sig) = &asts[0] {
        assert_eq!(lookup(*name), "add");
        assert!(sig.forany_vars.is_empty());
        assert!(sig.constraints.is_empty());
        if let TypeExpr::Function(_, params, ret) = &sig.fn_type {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], TypeExpr::Base(params[0].loc(), BaseTypeKind::Int));
            assert_eq!(params[1], TypeExpr::Base(params[1].loc(), BaseTypeKind::Int));
            assert_eq!(**ret, TypeExpr::Base(ret.loc(), BaseTypeKind::Int));
        } else {
            panic!("Expected function type, got {:?}", sig.fn_type);
        }
    } else {
        panic!("Expected TypeSignature, got {:?}", asts[0]);
    }
}

#[test]
fn test_parse_forany_and_higher_order_signature() {
    let mut diags = Vec::new();
    let src = "filter :: forany(A) : (A -> bool), [A] -> [A]";
    let asts = parse_all(src, intern("<test>"), &mut diags);
    assert!(diags.is_empty(), "diags: {:?}", diags);
    assert_eq!(asts.len(), 1);

    if let Ast::TypeSignature(_, name, sig) = &asts[0] {
        assert_eq!(lookup(*name), "filter");
        assert_eq!(sig.forany_vars.len(), 1);
        assert_eq!(lookup(sig.forany_vars[0]), "A");
        assert!(sig.constraints.is_empty());
        if let TypeExpr::Function(_, params, ret) = &sig.fn_type {
            assert_eq!(params.len(), 2);
            // param 0: (A -> bool)
            if let TypeExpr::Function(_, fn_params, fn_ret) = &params[0] {
                assert_eq!(fn_params.len(), 1);
                assert_eq!(fn_params[0], TypeExpr::Var(fn_params[0].loc(), sig.forany_vars[0]));
                assert_eq!(**fn_ret, TypeExpr::Base(fn_ret.loc(), BaseTypeKind::Bool));
            } else {
                panic!("Expected param 0 to be function, got {:?}", params[0]);
            }
            // param 1: [A]
            if let TypeExpr::List(_, elem) = &params[1] {
                assert_eq!(**elem, TypeExpr::Var(elem.loc(), sig.forany_vars[0]));
            } else {
                panic!("Expected param 1 to be list, got {:?}", params[1]);
            }
            // ret: [A]
            if let TypeExpr::List(_, elem) = &**ret {
                assert_eq!(**elem, TypeExpr::Var(elem.loc(), sig.forany_vars[0]));
            } else {
                panic!("Expected ret to be list, got {:?}", ret);
            }
        } else {
            panic!("Expected function type, got {:?}", sig.fn_type);
        }
    } else {
        panic!("Expected TypeSignature, got {:?}", asts[0]);
    }
}

#[test]
fn test_parse_trait_constraint_signature() {
    let mut diags = Vec::new();
    let src = "unique :: forany(A) where implements(A, :comparable) : [A] -> [A]";
    let asts = parse_all(src, intern("<test>"), &mut diags);
    assert!(diags.is_empty(), "diags: {:?}", diags);
    assert_eq!(asts.len(), 1);

    if let Ast::TypeSignature(_, name, sig) = &asts[0] {
        assert_eq!(lookup(*name), "unique");
        assert_eq!(sig.forany_vars.len(), 1);
        assert_eq!(lookup(sig.forany_vars[0]), "A");
        assert_eq!(sig.constraints.len(), 1);
        assert_eq!(lookup(sig.constraints[0].type_var), "A");
        assert_eq!(lookup(sig.constraints[0].trait_name), "comparable");
    } else {
        panic!("Expected TypeSignature, got {:?}", asts[0]);
    }
}

#[test]
fn test_parse_newtype_derive_and_implements() {
    let mut diags = Vec::new();
    let src = r#"
newtype Foo := { c: int }
derive(Foo, :comparable)
implements(Foo, :comparable, \a b -> a.c == b.c)
"#;
    let asts = parse_all(src, intern("<test>"), &mut diags);
    assert!(diags.is_empty(), "diags: {:?}", diags);
    assert_eq!(asts.len(), 3);

    // 1. newtype Foo := { c: int }
    if let Ast::Newtype(_, name, ty) = &asts[0] {
        assert_eq!(lookup(*name), "Foo");
        if let TypeExpr::Record(_, fields) = ty {
            assert_eq!(fields.len(), 1);
            assert_eq!(lookup(fields[0].0), "c");
            assert_eq!(fields[0].1, TypeExpr::Base(fields[0].1.loc(), BaseTypeKind::Int));
        } else {
            panic!("Expected record type, got {:?}", ty);
        }
    } else {
        panic!("Expected Newtype, got {:?}", asts[0]);
    }

    // 2. derive(Foo, :comparable)
    if let Ast::Derive(_, name, tr) = &asts[1] {
        assert_eq!(lookup(*name), "Foo");
        assert_eq!(lookup(*tr), "comparable");
    } else {
        panic!("Expected Derive, got {:?}", asts[1]);
    }

    // 3. implements(Foo, :comparable, ...)
    if let Ast::Implements(_, name, tr, handler) = &asts[2] {
        assert_eq!(lookup(*name), "Foo");
        assert_eq!(lookup(*tr), "comparable");
        assert!(matches!(**handler, Ast::Lambda(..)));
    } else {
        panic!("Expected Implements, got {:?}", asts[2]);
    }
}

#[test]
fn test_parse_inline_type_assert() {
    let mut diags = Vec::new();
    let src = "x := {c: 10} :: record";
    let asts = parse_all(src, intern("<test>"), &mut diags);
    assert!(diags.is_empty(), "diags: {:?}", diags);
    assert_eq!(asts.len(), 1);

    if let Ast::Define(_, name, val) = &asts[0] {
        assert_eq!(lookup(*name), "x");
        if let Ast::TypeAssert(_, expr, ty) = &**val {
            assert!(matches!(**expr, Ast::Record(..)));
            assert_eq!(*ty, TypeExpr::Base(ty.loc(), BaseTypeKind::Record));
        } else {
            panic!("Expected TypeAssert, got {:?}", val);
        }
    } else {
        panic!("Expected Define, got {:?}", asts[0]);
    }
}
