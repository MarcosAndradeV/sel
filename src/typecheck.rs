//! Static Type Checker for SEL
//!
//! Enforces compile-time type safety with a strict static gate (Option 1A):
//! - Universal quantification (`forany(A, B)`)
//! - Ad-hoc polymorphism & trait bounds (`where implements(A, :trait)`)
//! - Nominal types (`newtype Foo := { c: int }`) and constructors
//! - Mandatory signatures for `pub` exports in typed modules (Option B)
//! - Bidirectional type checking and type inference

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ast::*;
use crate::diagnostics::SelError;
use crate::lexer::Loc;
use crate::types::lookup;

/// Semantic representation of types in SEL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    Symbol,
    Nil,
    Any,
    Record(Vec<(u32, Type)>), // sorted by field symbol ID
    List(Box<Type>),
    Function(Vec<Type>, Box<Type>),
    Nominal(u32), // Symbol ID of nominal type name (e.g. Foo)
    Var(u32),     // Type variable symbol ID (e.g. A)
    Union(Vec<Type>),
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "string"),
            Type::Char => write!(f, "char"),
            Type::Symbol => write!(f, "symbol"),
            Type::Nil => write!(f, "nil"),
            Type::Any => write!(f, "any"),
            Type::List(elem) => write!(f, "[{}]", elem),
            Type::Record(fields) => {
                write!(f, "{{")?;
                for (i, (key, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", lookup(*key), ty)?;
                }
                write!(f, "}}")
            }
            Type::Nominal(id) => write!(f, "{}", lookup(*id)),
            Type::Var(id) => write!(f, "{}", lookup(*id)),
            Type::Function(params, ret) => {
                if params.is_empty() {
                    write!(f, "() -> {}", ret)
                } else {
                    for (i, p) in params.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        if matches!(p, Type::Function(..)) {
                            write!(f, "({})", p)?;
                        } else {
                            write!(f, "{}", p)?;
                        }
                    }
                    write!(f, " -> {}", ret)
                }
            }
            Type::Union(variants) => {
                for (i, v) in variants.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", v)?;
                }
                Ok(())
            }
        }
    }
}

impl Type {
    pub fn from_ast(expr: &TypeExpr) -> Self {
        match expr {
            TypeExpr::Base(_, base) => match base {
                BaseTypeKind::Int => Type::Int,
                BaseTypeKind::Float => Type::Float,
                BaseTypeKind::Bool => Type::Bool,
                BaseTypeKind::String => Type::String,
                BaseTypeKind::Char => Type::Char,
                BaseTypeKind::Symbol => Type::Symbol,
                BaseTypeKind::Nil => Type::Nil,
                BaseTypeKind::Any => Type::Any,
                BaseTypeKind::Record => Type::Record(Vec::new()),
                BaseTypeKind::List => Type::List(Box::new(Type::Any)),
                BaseTypeKind::Fn => Type::Function(vec![Type::Any], Box::new(Type::Any)),
            },
            TypeExpr::Var(_, id) => Type::Var(*id),
            TypeExpr::Nominal(_, id) => Type::Nominal(*id),
            TypeExpr::List(_, elem) => Type::List(Box::new(Type::from_ast(elem))),
            TypeExpr::Record(_, fields) => {
                let mut sorted_fields: Vec<(u32, Type)> = fields
                    .iter()
                    .map(|(k, v)| (*k, Type::from_ast(v)))
                    .collect();
                sorted_fields.sort_by_key(|(k, _)| *k);
                Type::Record(sorted_fields)
            }
            TypeExpr::Function(_, params, ret) => Type::Function(
                params.iter().map(Type::from_ast).collect(),
                Box::new(Type::from_ast(ret)),
            ),
            TypeExpr::Union(_, variants) => {
                Type::Union(variants.iter().map(Type::from_ast).collect())
            }
        }
    }

    /// Check if `self` can be assigned where `expected` is expected.
    pub fn is_assignable_to(&self, expected: &Type, env: &TypeEnv) -> bool {
        if self == expected || expected == &Type::Any || self == &Type::Any {
            return true;
        }

        match (self, expected) {
            (Type::Union(variants), _) => {
                variants.iter().all(|v| v.is_assignable_to(expected, env))
            }
            (_, Type::Union(variants)) => {
                variants.iter().any(|v| self.is_assignable_to(v, env))
            }
            (Type::List(elem1), Type::List(elem2)) => elem1.is_assignable_to(elem2, env),
            (Type::Record(f1), Type::Record(f2)) => {
                if f2.is_empty() {
                    return true; // generic record base type
                }
                // Structural width and depth subtyping: f1 must contain all fields of f2
                for (k2, ty2) in f2 {
                    if let Some((_, ty1)) = f1.iter().find(|(k1, _)| k1 == k2) {
                        if !ty1.is_assignable_to(ty2, env) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            (Type::Function(p1, r1), Type::Function(p2, r2)) => {
                if p1.len() != p2.len() {
                    return false;
                }
                if !r1.is_assignable_to(r2, env) {
                    return false;
                }
                // Contravariant parameter matching
                for (arg2, param1) in p2.iter().zip(p1.iter()) {
                    if !arg2.is_assignable_to(param1, env) {
                        return false;
                    }
                }
                true
            }
            (Type::Nominal(id1), Type::Nominal(id2)) => id1 == id2,
            _ => false,
        }
    }
}

/// Static Type Environment for a module compilation unit.
#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    pub newtypes: FxHashMap<u32, Type>,
    pub signatures: FxHashMap<u32, TypeSignature>,
    pub trait_derives: FxHashSet<(u32, u32)>,
    pub trait_impls: FxHashMap<(u32, u32), Ast>,
    pub scopes: Vec<FxHashMap<u32, Type>>,
    pub is_typed_module: bool,
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut env = Self::default();
        env.scopes.push(FxHashMap::default());
        env
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(FxHashMap::default());
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn insert_local(&mut self, name: u32, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    pub fn lookup_var(&self, name: u32) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(&name) {
                return Some(ty.clone());
            }
        }
        // Check declared signatures
        if let Some(sig) = self.signatures.get(&name) {
            return Some(Type::from_ast(&sig.fn_type));
        }
        // Check newtype constructor: Foo : Underlying -> Foo
        if let Some(underlying) = self.newtypes.get(&name) {
            return Some(Type::Function(
                vec![underlying.clone()],
                Box::new(Type::Nominal(name)),
            ));
        }
        None
    }

    /// Check if a concrete type implements the specified trait.
    pub fn implements_trait(&self, ty: &Type, trait_name: u32) -> bool {
        let tr_name = lookup(trait_name);
        match ty {
            Type::Int | Type::Float | Type::Bool | Type::String | Type::Char | Type::Symbol | Type::Nil => {
                tr_name == "comparable" || tr_name == "showable" || tr_name == "hashable"
            }
            Type::List(elem) => {
                if tr_name == "comparable" {
                    self.implements_trait(elem, trait_name)
                } else {
                    tr_name == "showable" || tr_name == "iterable"
                }
            }
            Type::Record(fields) => {
                if tr_name == "comparable" {
                    fields.iter().all(|(_, fty)| self.implements_trait(fty, trait_name))
                } else {
                    tr_name == "showable"
                }
            }
            Type::Nominal(id) => {
                self.trait_derives.contains(&(*id, trait_name))
                    || self.trait_impls.contains_key(&(*id, trait_name))
            }
            Type::Any => true,
            _ => false,
        }
    }
}

/// Typecheck an entire program (AST list).
/// If any type error occurs, returns `Err(SelError::TypeError)`.
pub fn typecheck_program(asts: &[Ast]) -> Result<(), SelError> {
    let mut env = TypeEnv::new();

    // 1. Scan if module has any type declarations or assertions
    for ast in asts {
        scan_has_types(ast, &mut env.is_typed_module);
    }

    // If it's a completely untyped legacy script, don't enforce mandatory signatures
    if !env.is_typed_module {
        return Ok(());
    }

    // 2. Discovery pass: collect newtypes, derive, implements, and signatures
    for ast in asts {
        collect_declarations(ast, &mut env)?;
    }

    // 3. Option B enforcement: check that all `pub` function definitions have explicit signatures
    check_pub_signatures(asts, &env)?;

    // 4. Verification pass: typecheck all statements and expressions
    for ast in asts {
        typecheck_ast(ast, &mut env)?;
    }

    Ok(())
}

fn scan_has_types(ast: &Ast, has_types: &mut bool) {
    if *has_types {
        return;
    }
    match ast {
        Ast::TypeSignature(..)
        | Ast::TypeAssert(..)
        | Ast::Newtype(..)
        | Ast::Derive(..)
        | Ast::Implements(..) => {
            *has_types = true;
        }
        Ast::Define(_, _, val)
        | Ast::Set(_, _, val)
        | Ast::DefMacro(_, _, val)
        | Ast::Quote(_, val)
        | Ast::Quasiquote(_, val)
        | Ast::Unquote(_, val)
        | Ast::UnquoteSplicing(_, val)
        | Ast::DotAccess(_, val, _)
        | Ast::Yield(_, val)
        | Ast::Load(_, val) => {
            scan_has_types(val, has_types);
        }
        Ast::If(_, c, t, e) | Ast::Unless(_, c, t, e) => {
            scan_has_types(c, has_types);
            scan_has_types(t, has_types);
            if let Some(eb) = e {
                scan_has_types(eb, has_types);
            }
        }
        Ast::When(_, c, body) => {
            scan_has_types(c, has_types);
            for b in body {
                scan_has_types(b, has_types);
            }
        }
        Ast::While(_, c, b) | Ast::Until(_, c, b) | Ast::CoResume(_, c, b) => {
            scan_has_types(c, has_types);
            scan_has_types(b, has_types);
        }
        Ast::Pipeline(_, l, r, _) => {
            scan_has_types(l, has_types);
            scan_has_types(r, has_types);
        }
        Ast::Try(_, body, _, catch) => {
            scan_has_types(body, has_types);
            for b in catch {
                scan_has_types(b, has_types);
            }
        }
        Ast::Lambda(_, _, _, body) => {
            for b in body {
                scan_has_types(b, has_types);
            }
        }
        Ast::Begin(_, exprs) | Ast::And(_, exprs) | Ast::Or(_, exprs) | Ast::List(_, exprs) => {
            for e in exprs {
                scan_has_types(e, has_types);
            }
        }
        Ast::Cond(_, branches) => {
            for (c, b) in branches {
                scan_has_types(c, has_types);
                scan_has_types(b, has_types);
            }
        }
        Ast::Let(_, bindings, body) => {
            for (_, v) in bindings {
                scan_has_types(v, has_types);
            }
            for b in body {
                scan_has_types(b, has_types);
            }
        }
        Ast::Record(_, fields) => {
            for (_, v) in fields {
                scan_has_types(v, has_types);
            }
        }
        Ast::Match(_, target, clauses) => {
            scan_has_types(target, has_types);
            for clause in clauses {
                if let Some(ref g) = clause.guard {
                    scan_has_types(g, has_types);
                }
                for b in &clause.body {
                    scan_has_types(b, has_types);
                }
            }
        }
        _ => {}
    }
}

fn collect_declarations(ast: &Ast, env: &mut TypeEnv) -> Result<(), SelError> {
    match ast {
        Ast::TypeSignature(_, name, sig) => {
            env.signatures.insert(*name, sig.clone());
        }
        Ast::Newtype(_, name, ty_expr) => {
            let ty = Type::from_ast(ty_expr);
            env.newtypes.insert(*name, ty);
        }
        Ast::Derive(_, name, tr) => {
            env.trait_derives.insert((*name, *tr));
        }
        Ast::Implements(_, name, tr, handler) => {
            env.trait_impls.insert((*name, *tr), *handler.clone());
        }
        Ast::Begin(_, items) => {
            for item in items {
                collect_declarations(item, env)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_pub_signatures(asts: &[Ast], env: &TypeEnv) -> Result<(), SelError> {
    for ast in asts {
        if let Ast::Begin(_loc, items) = ast {
            // Check for VisibilityDirective(true)
            if let Some(Ast::VisibilityDirective(_, true)) = items.first() {
                for item in items {
                    if let Ast::Define(def_loc, name, _) = item {
                        if !env.signatures.contains_key(name) && !env.newtypes.contains_key(name) {
                            return Err(SelError::TypeError(
                                *def_loc,
                                format!(
                                    "Public function `{}` requires an explicit type signature `{} :: ...`",
                                    lookup(*name),
                                    lookup(*name)
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn typecheck_ast(ast: &Ast, env: &mut TypeEnv) -> Result<Type, SelError> {
    match ast {
        Ast::TypeSignature(..)
        | Ast::Newtype(..)
        | Ast::Derive(..)
        | Ast::Implements(..)
        | Ast::Import(..)
        | Ast::VisibilityDirective(..) => Ok(Type::Nil),

        Ast::Integer(..) => Ok(Type::Int),
        Ast::Float(..) => Ok(Type::Float),
        Ast::Boolean(..) => Ok(Type::Bool),
        Ast::String(..) => Ok(Type::String),
        Ast::Char(..) => Ok(Type::Char),
        Ast::Nil(..) => Ok(Type::Nil),
        Ast::Quote(_, inner) => {
            if let Ast::Symbol(..) = &**inner {
                Ok(Type::Symbol)
            } else {
                Ok(Type::Any)
            }
        }

        Ast::Symbol(_loc, id) => {
            if let Some(ty) = env.lookup_var(*id) {
                Ok(ty)
            } else {
                // If untyped variable, default to Any
                Ok(Type::Any)
            }
        }

        Ast::TypeAssert(loc, expr, ty_expr) => {
            let expected_ty = Type::from_ast(ty_expr);
            let actual_ty = typecheck_ast(expr, env)?;
            if !actual_ty.is_assignable_to(&expected_ty, env) {
                return Err(SelError::TypeError(
                    *loc,
                    format!(
                        "Type assertion failed: expected `{}`, found `{}`",
                        expected_ty, actual_ty
                    ),
                ));
            }
            Ok(expected_ty)
        }

        Ast::Record(_, fields) => {
            let mut ty_fields = Vec::new();
            for (key, val) in fields {
                let ty = typecheck_ast(val, env)?;
                ty_fields.push((*key, ty));
            }
            ty_fields.sort_by_key(|(k, _)| *k);
            Ok(Type::Record(ty_fields))
        }

        Ast::List(loc, items) => {
            if items.is_empty() {
                return Ok(Type::List(Box::new(Type::Any)));
            }

            // Check if it's a list literal: `[item1, item2, ...]` which is desugared as `(list item1 item2 ...)`
            if let Ast::Symbol(_, sym_id) = &items[0] {
                if lookup(*sym_id) == "list" {
                    let list_elems = &items[1..];
                    if list_elems.is_empty() {
                        return Ok(Type::List(Box::new(Type::Any)));
                    }
                    let first_ty = typecheck_ast(&list_elems[0], env)?;
                    for elem in &list_elems[1..] {
                        let next_ty = typecheck_ast(elem, env)?;
                        if !next_ty.is_assignable_to(&first_ty, env) && !first_ty.is_assignable_to(&next_ty, env) {
                            return Ok(Type::List(Box::new(Type::Any)));
                        }
                    }
                    return Ok(Type::List(Box::new(first_ty)));
                }
            }

            // Otherwise, it's a function or constructor call!
            let callee_ast = &items[0];
            let callee_ty = typecheck_ast(callee_ast, env)?;

            let mut arg_types = Vec::new();
            for arg in &items[1..] {
                arg_types.push(typecheck_ast(arg, env)?);
            }

            match callee_ty {
                Type::Function(param_types, ret_ty) => {
                    // Check if function was a generic signature
                    if let Ast::Symbol(_, sym_id) = callee_ast {
                        if let Some(sig) = env.signatures.get(sym_id).cloned() {
                            return check_generic_call(*loc, &sig, &arg_types, env);
                        }
                    }

                    // Concrete function call
                    if param_types.len() != arg_types.len() {
                        return Err(SelError::TypeError(
                            *loc,
                            format!(
                                "Function call arity mismatch: expected {} arguments, found {}",
                                param_types.len(),
                                arg_types.len()
                            ),
                        ));
                    }

                    for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                        if !actual.is_assignable_to(expected, env) {
                            return Err(SelError::TypeError(
                                *loc,
                                format!(
                                    "Type mismatch in argument {}: expected `{}`, found `{}`",
                                    i + 1,
                                    expected,
                                    actual
                                ),
                            ));
                        }
                    }
                    Ok(*ret_ty)
                }
                Type::Any => Ok(Type::Any),
                _ => Ok(Type::Any),
            }
        }

        Ast::DotAccess(loc, expr, field_sym) => {
            let expr_ty = typecheck_ast(expr, env)?;
            match expr_ty {
                Type::Record(fields) => {
                    if let Some((_, fty)) = fields.iter().find(|(k, _)| k == field_sym) {
                        Ok(fty.clone())
                    } else {
                        Err(SelError::TypeError(
                            *loc,
                            format!(
                                "Field `{}` not found on record of type `{}`",
                                lookup(*field_sym),
                                Type::Record(fields)
                            ),
                        ))
                    }
                }
                Type::Nominal(nom_id) => {
                    // Transparent dot-access on nominal types!
                    if let Some(Type::Record(fields)) = env.newtypes.get(&nom_id) {
                        if let Some((_, fty)) = fields.iter().find(|(k, _)| k == field_sym) {
                            Ok(fty.clone())
                        } else {
                            Err(SelError::TypeError(
                                *loc,
                                format!(
                                    "Field `{}` not found on nominal type `{}`",
                                    lookup(*field_sym),
                                    lookup(nom_id)
                                ),
                            ))
                        }
                    } else {
                        Ok(Type::Any)
                    }
                }
                Type::Any => Ok(Type::Any),
                other => Err(SelError::TypeError(
                    *loc,
                    format!("Cannot access field `{}` on type `{}`", lookup(*field_sym), other),
                )),
            }
        }

        Ast::Pipeline(loc, left, right, kind) => {
            let lowered = crate::compiler::lower_pipeline(*loc, *left.clone(), *right.clone(), *kind);
            typecheck_ast(&lowered, env)
        }

        Ast::Define(_loc, name, val) => {
            if let Some(sig) = env.signatures.get(name).cloned() {
                // Check value against signature
                check_with_signature(val, &sig, env)?;
                Ok(Type::Nil)
            } else {
                let inferred = typecheck_ast(val, env)?;
                env.insert_local(*name, inferred);
                Ok(Type::Nil)
            }
        }

        Ast::Lambda(_loc, _, params, body) => {
            env.enter_scope();
            for p in params {
                env.insert_local(*p, Type::Any);
            }
            let mut ret_ty = Type::Nil;
            for stmt in body {
                ret_ty = typecheck_ast(stmt, env)?;
            }
            env.exit_scope();
            let param_types = vec![Type::Any; params.len()];
            Ok(Type::Function(param_types, Box::new(ret_ty)))
        }

        Ast::Begin(_, items) => {
            let mut last_ty = Type::Nil;
            for item in items {
                last_ty = typecheck_ast(item, env)?;
            }
            Ok(last_ty)
        }

        Ast::Let(_loc, bindings, body) => {
            env.enter_scope();
            for (name, val) in bindings {
                let ty = typecheck_ast(val, env)?;
                env.insert_local(*name, ty);
            }
            let mut ret_ty = Type::Nil;
            for stmt in body {
                ret_ty = typecheck_ast(stmt, env)?;
            }
            env.exit_scope();
            Ok(ret_ty)
        }

        Ast::If(loc, cond, then_b, else_b) => {
            let cond_ty = typecheck_ast(cond, env)?;
            if !cond_ty.is_assignable_to(&Type::Bool, env) && cond_ty != Type::Any {
                return Err(SelError::TypeError(
                    *loc,
                    format!("Condition in `if` must be `bool`, found `{}`", cond_ty),
                ));
            }
            let then_ty = typecheck_ast(then_b, env)?;
            if let Some(eb) = else_b {
                let else_ty = typecheck_ast(eb, env)?;
                if then_ty.is_assignable_to(&else_ty, env) {
                    Ok(else_ty)
                } else if else_ty.is_assignable_to(&then_ty, env) {
                    Ok(then_ty)
                } else {
                    Ok(Type::Union(vec![then_ty, else_ty]))
                }
            } else {
                Ok(then_ty)
            }
        }

        Ast::Match(_, target, clauses) => {
            let _target_ty = typecheck_ast(target, env)?;
            let mut ret_ty = Type::Nil;
            for clause in clauses {
                env.enter_scope();
                bind_pattern_types(&clause.pattern, env);
                if let Some(ref guard) = clause.guard {
                    typecheck_ast(guard, env)?;
                }
                for stmt in &clause.body {
                    ret_ty = typecheck_ast(stmt, env)?;
                }
                env.exit_scope();
            }
            Ok(ret_ty)
        }

        _ => Ok(Type::Any),
    }
}

fn bind_pattern_types(pat: &Pattern, env: &mut TypeEnv) {
    match pat {
        Pattern::Variable(_, name) => {
            env.insert_local(*name, Type::Any);
        }
        Pattern::As(_, name, sub) => {
            env.insert_local(*name, Type::Any);
            bind_pattern_types(sub, env);
        }
        Pattern::List(_, pats) | Pattern::Or(_, pats) => {
            for p in pats {
                bind_pattern_types(p, env);
            }
        }
        Pattern::Cons(_, h, t) => {
            bind_pattern_types(h, env);
            bind_pattern_types(t, env);
        }
        Pattern::Rest(_, pats, tail) => {
            for p in pats {
                bind_pattern_types(p, env);
            }
            bind_pattern_types(tail, env);
        }
        Pattern::Record(_, fields) => {
            for (_, p) in fields {
                bind_pattern_types(p, env);
            }
        }
        _ => {}
    }
}

fn check_with_signature(ast: &Ast, sig: &TypeSignature, env: &mut TypeEnv) -> Result<(), SelError> {
    let fn_ty = Type::from_ast(&sig.fn_type);
    if let Type::Function(expected_params, expected_ret) = fn_ty {
        match ast {
            Ast::Lambda(loc, _, params, body) => {
                if params.len() != expected_params.len() {
                    return Err(SelError::TypeError(
                        *loc,
                        format!(
                            "Function arity mismatch with signature: expected {} parameters, found {}",
                            expected_params.len(),
                            params.len()
                        ),
                    ));
                }
                env.enter_scope();
                for (p_name, p_ty) in params.iter().zip(expected_params.iter()) {
                    env.insert_local(*p_name, p_ty.clone());
                }
                let mut actual_ret = Type::Nil;
                for stmt in body {
                    actual_ret = typecheck_ast(stmt, env)?;
                }
                env.exit_scope();

                if !actual_ret.is_assignable_to(&expected_ret, env) {
                    return Err(SelError::TypeError(
                        *loc,
                        format!(
                            "Function return type mismatch: expected `{}`, found `{}`",
                            expected_ret, actual_ret
                        ),
                    ));
                }
            }
            _ => {
                let actual = typecheck_ast(ast, env)?;
                if !actual.is_assignable_to(&Type::Function(expected_params, expected_ret.clone()), env) {
                    return Err(SelError::TypeError(
                        ast.loc(),
                        format!(
                            "Definition type mismatch: expected `{}`, found `{}`",
                            expected_ret, actual
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn check_generic_call(
    loc: Loc,
    sig: &TypeSignature,
    args: &[Type],
    env: &TypeEnv,
) -> Result<Type, SelError> {
    let sig_fn = Type::from_ast(&sig.fn_type);
    if let Type::Function(expected_params, ret_ty) = sig_fn {
        if expected_params.len() != args.len() {
            return Err(SelError::TypeError(
                loc,
                format!(
                    "Function call arity mismatch: expected {} arguments, found {}",
                    expected_params.len(),
                    args.len()
                ),
            ));
        }

        // Unify type variables from forany
        let mut substitutions: FxHashMap<u32, Type> = FxHashMap::default();
        for (expected, actual) in expected_params.iter().zip(args.iter()) {
            unify(expected, actual, &mut substitutions);
        }

        // Verify trait constraints from `where implements(...)`
        for constraint in &sig.constraints {
            if let Some(concrete_ty) = substitutions.get(&constraint.type_var) {
                if !env.implements_trait(concrete_ty, constraint.trait_name) {
                    return Err(SelError::TypeError(
                        loc,
                        format!(
                            "Type `{}` does not implement required trait `:{}`",
                            concrete_ty,
                            lookup(constraint.trait_name)
                        ),
                    ));
                }
            }
        }

        // Verify that arguments match expected parameters after substitution
        for (i, (expected, actual)) in expected_params.iter().zip(args.iter()).enumerate() {
            let substituted = substitute(expected, &substitutions);
            if !actual.is_assignable_to(&substituted, env) {
                return Err(SelError::TypeError(
                    loc,
                    format!(
                        "Type mismatch in argument {}: expected `{}`, found `{}`",
                        i + 1,
                        substituted,
                        actual
                    ),
                ));
            }
        }

        // Substitute into return type
        Ok(substitute(&ret_ty, &substitutions))
    } else {
        Ok(Type::Any)
    }
}

fn unify(expected: &Type, actual: &Type, subs: &mut FxHashMap<u32, Type>) {
    match (expected, actual) {
        (Type::Var(id), concrete) => {
            if !subs.contains_key(id) {
                subs.insert(*id, concrete.clone());
            }
        }
        (Type::List(e1), Type::List(e2)) => {
            unify(e1, e2, subs);
        }
        (Type::Function(p1, r1), Type::Function(p2, r2)) => {
            for (arg1, arg2) in p1.iter().zip(p2.iter()) {
                unify(arg1, arg2, subs);
            }
            unify(r1, r2, subs);
        }
        (Type::Record(f1), Type::Record(f2)) => {
            for (k1, ty1) in f1 {
                if let Some((_, ty2)) = f2.iter().find(|(k2, _)| k1 == k2) {
                    unify(ty1, ty2, subs);
                }
            }
        }
        _ => {}
    }
}

fn substitute(ty: &Type, subs: &FxHashMap<u32, Type>) -> Type {
    match ty {
        Type::Var(id) => subs.get(id).cloned().unwrap_or_else(|| Type::Var(*id)),
        Type::List(elem) => Type::List(Box::new(substitute(elem, subs))),
        Type::Function(params, ret) => Type::Function(
            params.iter().map(|p| substitute(p, subs)).collect(),
            Box::new(substitute(ret, subs)),
        ),
        Type::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|(k, v)| (*k, substitute(v, subs)))
                .collect(),
        ),
        Type::Union(variants) => {
            Type::Union(variants.iter().map(|v| substitute(v, subs)).collect())
        }
        other => other.clone(),
    }
}
