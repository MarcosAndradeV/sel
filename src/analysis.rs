//! Static pattern matching analysis for SEL.
//!
//! Implements reachability (redundant pattern detection) and exhaustiveness checking
//! based on the usefulness algorithm (Maranget, 2007) adapted for SEL's dynamic value universe.

use rustc_hash::FxHashSet;
use crate::ast::{Ast, MatchClause, Pattern};
use crate::diagnostics::SelWarning;
use crate::lexer::Loc;
use crate::types::lookup;

/// Representation of a simplified constructor for pattern usefulness analysis.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ctor {
    Wildcard,
    Boolean(bool),
    Nil,
    EmptyList,
    NonEmptyList,
    Atom(u32),
    Integer(i64),
    Float(u64), // f64 bits
    String(String),
    Char(char),
    Record(Vec<u32>), // required record keys (sorted)
}

/// A simplified pattern matrix cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimplePat {
    Wildcard,
    Ctor(Ctor, Vec<SimplePat>),
    Or(Vec<SimplePat>),
}

impl SimplePat {
    pub fn is_wildcard(&self) -> bool {
        matches!(self, SimplePat::Wildcard)
    }

    /// Recursively expand or-patterns or as-patterns into simpler patterns.
    pub fn from_pattern(pat: &Pattern) -> SimplePat {
        match pat {
            Pattern::Wildcard(_) | Pattern::Variable(_, _) => SimplePat::Wildcard,
            Pattern::As(_, _, sub) => SimplePat::from_pattern(sub),
            Pattern::Literal(_, ast) => match &**ast {
                Ast::Boolean(_, b) => SimplePat::Ctor(Ctor::Boolean(*b), Vec::new()),
                Ast::Nil(_) => SimplePat::Ctor(Ctor::Nil, Vec::new()),
                Ast::Integer(_, i) => SimplePat::Ctor(Ctor::Integer(*i), Vec::new()),
                Ast::Float(_, f) => SimplePat::Ctor(Ctor::Float(f.to_bits()), Vec::new()),
                Ast::String(_, s) => SimplePat::Ctor(Ctor::String(s.clone()), Vec::new()),
                Ast::Char(_, c) => SimplePat::Ctor(Ctor::Char(*c), Vec::new()),
                Ast::Quote(_, inner) => {
                    if let Ast::Symbol(_, sym) = &**inner {
                        SimplePat::Ctor(Ctor::Atom(*sym), Vec::new())
                    } else {
                        SimplePat::Wildcard
                    }
                }
                _ => SimplePat::Wildcard,
            },
            Pattern::Cons(_, head, tail) => {
                let h = SimplePat::from_pattern(head);
                let t = SimplePat::from_pattern(tail);
                SimplePat::Ctor(Ctor::NonEmptyList, vec![h, t])
            }
            Pattern::List(_, elements) => {
                if elements.is_empty() {
                    SimplePat::Ctor(Ctor::EmptyList, Vec::new())
                } else {
                    // [e0, e1, ...] is cons(e0, cons(e1, ... cons(en, [])...))
                    let mut curr = SimplePat::Ctor(Ctor::EmptyList, Vec::new());
                    for elem in elements.iter().rev() {
                        let h = SimplePat::from_pattern(elem);
                        curr = SimplePat::Ctor(Ctor::NonEmptyList, vec![h, curr]);
                    }
                    curr
                }
            }
            Pattern::Rest(_, prefix, tail) => {
                let mut curr = SimplePat::from_pattern(tail);
                for elem in prefix.iter().rev() {
                    let h = SimplePat::from_pattern(elem);
                    curr = SimplePat::Ctor(Ctor::NonEmptyList, vec![h, curr]);
                }
                curr
            }
            Pattern::Record(_, fields) => {
                let mut sorted_fields: Vec<(u32, &Pattern)> =
                    fields.iter().map(|(k, p)| (*k, p)).collect();
                sorted_fields.sort_by_key(|&(k, _)| k);
                let keys: Vec<u32> = sorted_fields.iter().map(|&(k, _)| k).collect();
                let sub_pats: Vec<SimplePat> = sorted_fields
                    .iter()
                    .map(|&(_, p)| SimplePat::from_pattern(p))
                    .collect();
                SimplePat::Ctor(Ctor::Record(keys), sub_pats)
            }
            Pattern::Or(_, pats) => {
                let sub: Vec<SimplePat> = pats.iter().map(SimplePat::from_pattern).collect();
                SimplePat::Or(sub)
            }
        }
    }
}

/// A matrix of pattern rows, where each row has a list of patterns (for tuple matching or single target)
type Matrix = Vec<Vec<SimplePat>>;

/// Check if a vector pattern `row` is useful with respect to `matrix`.
/// A row is useful if there is a value matched by `row` that is not matched
/// by any previous row in `matrix`.
pub fn is_useful(matrix: &[Vec<SimplePat>], row: &[SimplePat]) -> bool {
    if matrix.is_empty() {
        return true;
    }

    // If the matrix already has an unguarded row of all wildcards,
    // then that row matched all possible dynamic values, so nothing after it is useful.
    if matrix.iter().any(|r| r.iter().all(|p| p.is_wildcard())) {
        return false;
    }

    if row.is_empty() {
        return false;
    }

    let first = &row[0];
    let rest = &row[1..];

    match first {
        SimplePat::Or(alternatives) => {
            // q = (p1 | p2 | ...) :: rest is useful if any alternative is useful
            alternatives.iter().any(|alt| {
                let mut new_row = vec![alt.clone()];
                new_row.extend_from_slice(rest);
                is_useful(matrix, &new_row)
            })
        }
        SimplePat::Ctor(ctor, args) => {
            // Specialized matrix for this constructor
            let s_matrix = specialize_matrix(matrix, ctor, args.len());
            let mut new_row = args.clone();
            new_row.extend_from_slice(rest);
            is_useful(&s_matrix, &new_row)
        }
        SimplePat::Wildcard => {
            let (ctors, has_finite_domain) = collect_column_ctors(matrix);
            if has_finite_domain {
                ctors.iter().any(|ctor| {
                    let arity = match ctor {
                        Ctor::NonEmptyList => 2,
                        Ctor::Record(keys) => keys.len(),
                        _ => 0,
                    };
                    let s_matrix = specialize_matrix(matrix, ctor, arity);
                    let mut new_row = vec![SimplePat::Wildcard; arity];
                    new_row.extend_from_slice(rest);
                    is_useful(&s_matrix, &new_row)
                })
            } else {
                let d_matrix = default_matrix(matrix);
                is_useful(&d_matrix, rest)
            }
        }
    }
}



/// Collects distinct constructors appearing in the first column of the matrix,
/// and determines if the domain of this column is finite and completely enumerated.
fn collect_column_ctors(matrix: &[Vec<SimplePat>]) -> (Vec<Ctor>, bool) {
    let mut ctors: Vec<Ctor> = Vec::new();
    let mut seen = FxHashSet::default();

    fn extract_ctors(p: &SimplePat, acc: &mut Vec<Ctor>, seen: &mut FxHashSet<Ctor>) {
        match p {
            SimplePat::Ctor(c, _) => {
                if seen.insert(c.clone()) {
                    acc.push(c.clone());
                }
            }
            SimplePat::Or(subs) => {
                for s in subs {
                    extract_ctors(s, acc, seen);
                }
            }
            SimplePat::Wildcard => {}
        }
    }

    for row in matrix {
        if !row.is_empty() {
            extract_ctors(&row[0], &mut ctors, &mut seen);
        }
    }

    let has_true = seen.contains(&Ctor::Boolean(true));
    let has_false = seen.contains(&Ctor::Boolean(false));
    let only_booleans = seen.iter().all(|c| matches!(c, Ctor::Boolean(_)));
    if only_booleans && has_true && has_false {
        return (vec![Ctor::Boolean(true), Ctor::Boolean(false)], true);
    }

    let has_empty = seen.contains(&Ctor::EmptyList);
    let has_non_empty = seen.contains(&Ctor::NonEmptyList);
    let only_lists = seen.iter().all(|c| matches!(c, Ctor::EmptyList | Ctor::NonEmptyList));
    if only_lists && has_empty && has_non_empty {
        return (vec![Ctor::EmptyList, Ctor::NonEmptyList], true);
    }

    (ctors, false)
}

/// Specialize matrix M for constructor C with given arity.
fn specialize_matrix(matrix: &[Vec<SimplePat>], ctor: &Ctor, arity: usize) -> Matrix {
    let mut res = Vec::new();

    for row in matrix {
        if row.is_empty() {
            continue;
        }
        let first = &row[0];
        let rest = &row[1..];

        match first {
            SimplePat::Ctor(c, args) => {
                if c == ctor {
                    let mut new_row = args.clone();
                    new_row.extend_from_slice(rest);
                    res.push(new_row);
                }
            }
            SimplePat::Wildcard => {
                let mut new_row = vec![SimplePat::Wildcard; arity];
                new_row.extend_from_slice(rest);
                res.push(new_row);
            }
            SimplePat::Or(alts) => {
                for alt in alts {
                    let mut single_row = vec![alt.clone()];
                    single_row.extend_from_slice(rest);
                    let mut spec = specialize_matrix(&[single_row], ctor, arity);
                    res.append(&mut spec);
                }
            }
        }
    }

    res
}

/// Default matrix D(M) removes rows starting with constructors and strips wildcards.
fn default_matrix(matrix: &[Vec<SimplePat>]) -> Matrix {
    let mut res = Vec::new();

    for row in matrix {
        if row.is_empty() {
            continue;
        }
        let first = &row[0];
        let rest = &row[1..];

        match first {
            SimplePat::Wildcard => {
                res.push(rest.to_vec());
            }
            SimplePat::Or(alts) => {
                for alt in alts {
                    let mut single_row = vec![alt.clone()];
                    single_row.extend_from_slice(rest);
                    let mut def = default_matrix(&[single_row]);
                    res.append(&mut def);
                }
            }
            SimplePat::Ctor(..) => {}
        }
    }

    res
}

/// Cartesian product expansion of or-patterns in a single row.
fn expand_or_row(row: &[SimplePat]) -> Vec<Vec<SimplePat>> {
    if row.is_empty() {
        return vec![vec![]];
    }
    let rest_expanded = expand_or_row(&row[1..]);
    match &row[0] {
        SimplePat::Or(alts) => {
            let mut res = Vec::new();
            for alt in alts {
                for rest in &rest_expanded {
                    let mut r = vec![alt.clone()];
                    r.extend_from_slice(rest);
                    res.push(r);
                }
            }
            res
        }
        other => {
            let mut res = Vec::new();
            for rest in &rest_expanded {
                let mut r = vec![other.clone()];
                r.extend_from_slice(rest);
                res.push(r);
            }
            res
        }
    }
}

/// Try to unpack a pattern into `arity` sub-patterns for tuple/multi-argument matching.
fn unpack_clause_pattern(pat: &Pattern, arity: usize) -> Option<Vec<SimplePat>> {
    match pat {
        Pattern::List(_, elements) if elements.len() == arity => {
            Some(elements.iter().map(SimplePat::from_pattern).collect())
        }
        Pattern::As(_, _, sub) => unpack_clause_pattern(sub, arity),
        Pattern::Wildcard(_) | Pattern::Variable(..) => {
            Some(vec![SimplePat::Wildcard; arity])
        }
        _ => None,
    }
}

fn is_list_call(ast: &Ast) -> Option<usize> {
    if let Ast::List(_, items) = ast {
        if let Some(Ast::Symbol(_, sym)) = items.first() {
            if lookup(*sym) == "list" {
                return Some(items.len().saturating_sub(1));
            }
        }
    }
    None
}

/// Checks if a pattern matrix is exhaustive, and returns any missing witness example.
pub fn check_matrix_exhaustiveness(matrix: &[Vec<SimplePat>], arity: usize) -> (bool, String) {
    if matrix.iter().any(|r| r.iter().all(|p| p.is_wildcard())) {
        return (true, String::new());
    }

    if matrix.is_empty() {
        return (false, "_".to_string());
    }

    if arity == 1 {
        let (ctors, _) = collect_column_ctors(matrix);

        // 1. Booleans
        let has_true = ctors.contains(&Ctor::Boolean(true));
        let has_false = ctors.contains(&Ctor::Boolean(false));
        let only_booleans = ctors.iter().all(|c| matches!(c, Ctor::Boolean(_)));

        if only_booleans && has_true && has_false {
            return (true, String::new());
        }
        if only_booleans && has_true && !has_false {
            return (false, "false".to_string());
        }
        if only_booleans && has_false && !has_true {
            return (false, "true".to_string());
        }

        // 2. Lists
        let has_empty = ctors.contains(&Ctor::EmptyList);
        let has_non_empty = ctors.contains(&Ctor::NonEmptyList);
        let only_lists = ctors.iter().all(|c| matches!(c, Ctor::EmptyList | Ctor::NonEmptyList));

        if only_lists {
            if has_empty && has_non_empty {
                let s_matrix = specialize_matrix(matrix, &Ctor::NonEmptyList, 2);
                if !is_useful(&s_matrix, &[SimplePat::Wildcard, SimplePat::Wildcard]) {
                    return (true, String::new());
                }
            }
            if has_empty && !has_non_empty {
                return (false, "[_ | _]".to_string());
            }
            if !has_empty && has_non_empty {
                return (false, "[]".to_string());
            }
        }

        return (false, "_".to_string());
    }

    // If all rows have wildcard in column 0, delegate to rest
    if matrix.iter().all(|r| !r.is_empty() && r[0].is_wildcard()) {
        let d_matrix = default_matrix(matrix);
        return check_matrix_exhaustiveness(&d_matrix, arity - 1);
    }

    // For multi-column patterns: check if column 0 covers lists or booleans
    let (col0_ctors, _) = collect_column_ctors(matrix);
    let col0_only_lists = col0_ctors.iter().all(|c| matches!(c, Ctor::EmptyList | Ctor::NonEmptyList));
    if col0_only_lists && col0_ctors.contains(&Ctor::EmptyList) && col0_ctors.contains(&Ctor::NonEmptyList) {
        let empty_spec = specialize_matrix(matrix, &Ctor::EmptyList, 0);
        let (empty_ex, _) = check_matrix_exhaustiveness(&empty_spec, arity - 1);

        let cons_spec = specialize_matrix(matrix, &Ctor::NonEmptyList, 2);
        let (cons_ex, _) = check_matrix_exhaustiveness(&cons_spec, arity + 1);

        if empty_ex && cons_ex {
            return (true, String::new());
        }
    }

    (false, "_".to_string())
}

/// Checks a sequence of match clauses for reachability and exhaustiveness.
/// Returns a list of compiler warnings for unreachable or inexhaustive clauses.
pub fn analyze_match(loc: Loc, target: Option<&Ast>, clauses: &[MatchClause]) -> Vec<SelWarning> {
    let mut warnings = Vec::new();

    // Check if target is a fixed-arity tuple/list of N arguments
    let arity_opt = target.and_then(is_list_call);
    let (matrix_rows, arity) = if let Some(arity) = arity_opt {
        if arity > 0 && clauses.iter().all(|c| unpack_clause_pattern(&c.pattern, arity).is_some()) {
            let unpacked: Vec<Vec<SimplePat>> = clauses
                .iter()
                .map(|c| unpack_clause_pattern(&c.pattern, arity).unwrap())
                .collect();
            (unpacked, arity)
        } else {
            let single: Vec<Vec<SimplePat>> = clauses
                .iter()
                .map(|c| vec![SimplePat::from_pattern(&c.pattern)])
                .collect();
            (single, 1)
        }
    } else {
        let single: Vec<Vec<SimplePat>> = clauses
            .iter()
            .map(|c| vec![SimplePat::from_pattern(&c.pattern)])
            .collect();
        (single, 1)
    };

    let mut matrix: Matrix = Vec::new();

    for (i, clause) in clauses.iter().enumerate() {
        let row = &matrix_rows[i];
        let is_guarded = clause.guard.is_some();

        let useful = if row.iter().all(|p| p.is_wildcard()) {
            !matrix.iter().any(|r| r.iter().all(|p| p.is_wildcard()))
        } else {
            is_useful(&matrix, row)
        };

        if !useful {
            warnings.push(SelWarning::UnreachablePattern(
                clause.loc,
                format!("clause `{}` will never be matched", format_pattern_summary(&clause.pattern)),
            ));
        }

        // Only add unguarded patterns to the matrix for future reachability checks
        if !is_guarded {
            let expanded_rows = expand_or_row(row);
            matrix.extend(expanded_rows);
        }
    }

    // Check exhaustiveness
    let (is_exhaustive, missing) = check_matrix_exhaustiveness(&matrix, arity);
    if !is_exhaustive {
        warnings.push(SelWarning::InexhaustivePattern(
            loc,
            format!("match is not exhaustive; missing case: {}", missing),
        ));
    }

    warnings
}

fn format_pattern_summary(pat: &Pattern) -> String {
    match pat {
        Pattern::Wildcard(_) => "_".to_string(),
        Pattern::Variable(_, id) => lookup(*id),
        Pattern::Literal(_, ast) => format!("{ast}"),
        Pattern::List(_, elements) => {
            let s: Vec<String> = elements.iter().map(format_pattern_summary).collect();
            format!("[{}]", s.join(", "))
        }
        Pattern::Cons(_, h, t) => {
            format!("[{} | {}]", format_pattern_summary(h), format_pattern_summary(t))
        }
        Pattern::Rest(_, pfx, rest) => {
            let s: Vec<String> = pfx.iter().map(format_pattern_summary).collect();
            format!("[{} | {}]", s.join(", "), format_pattern_summary(rest))
        }
        Pattern::Record(_, fields) => {
            let f: Vec<String> = fields
                .iter()
                .map(|(k, p)| format!("{}: {}", lookup(*k), format_pattern_summary(p)))
                .collect();
            format!("{{{}}}", f.join(", "))
        }
        Pattern::Or(_, pats) => {
            let s: Vec<String> = pats.iter().map(format_pattern_summary).collect();
            format!("({})", s.join(" | "))
        }
        Pattern::As(_, id, sub) => {
            format!("{} @ {}", lookup(*id), format_pattern_summary(sub))
        }
    }
}

/// Recursively analyze all pattern matching expressions within an AST for reachability and exhaustiveness.
pub fn analyze_ast(ast: &Ast, warnings: &mut Vec<SelWarning>) {
    match ast {
        Ast::Define(_, _, body) | Ast::DefMacro(_, _, body) | Ast::Set(_, _, body) => {
            analyze_ast(body, warnings);
        }
        Ast::Let(_, bindings, body) => {
            for (_, val) in bindings {
                analyze_ast(val, warnings);
            }
            for expr in body {
                analyze_ast(expr, warnings);
            }
        }
        Ast::If(_, cond, then_b, else_b) | Ast::Unless(_, cond, then_b, else_b) => {
            analyze_ast(cond, warnings);
            analyze_ast(then_b, warnings);
            if let Some(eb) = else_b {
                analyze_ast(eb, warnings);
            }
        }
        Ast::When(_, cond, body) => {
            analyze_ast(cond, warnings);
            for expr in body {
                analyze_ast(expr, warnings);
            }
        }
        Ast::Cond(_, branches) => {
            for (test, expr) in branches {
                analyze_ast(test, warnings);
                analyze_ast(expr, warnings);
            }
        }
        Ast::While(_, cond, body) | Ast::Until(_, cond, body) => {
            analyze_ast(cond, warnings);
            analyze_ast(body, warnings);
        }
        Ast::Lambda(_, _, _, body) | Ast::Begin(_, body) => {
            for expr in body {
                analyze_ast(expr, warnings);
            }
        }
        Ast::Quote(_, expr)
        | Ast::Quasiquote(_, expr)
        | Ast::Unquote(_, expr)
        | Ast::UnquoteSplicing(_, expr) => {
            analyze_ast(expr, warnings);
        }
        Ast::And(_, exprs) | Ast::Or(_, exprs) | Ast::List(_, exprs) => {
            for expr in exprs {
                analyze_ast(expr, warnings);
            }
        }
        Ast::Record(_, fields) => {
            for (_, val) in fields {
                analyze_ast(val, warnings);
            }
        }
        Ast::DotAccess(_, expr, _) => {
            analyze_ast(expr, warnings);
        }
        Ast::Pipeline(_, left, right, _) => {
            analyze_ast(left, warnings);
            analyze_ast(right, warnings);
        }
        Ast::Try(_, body, _, catch_body) => {
            analyze_ast(body, warnings);
            for expr in catch_body {
                analyze_ast(expr, warnings);
            }
        }
        Ast::Yield(_, expr) => {
            analyze_ast(expr, warnings);
        }
        Ast::CoResume(_, co, arg) => {
            analyze_ast(co, warnings);
            analyze_ast(arg, warnings);
        }
        Ast::Load(_, path) => {
            analyze_ast(path, warnings);
        }
        Ast::Match(loc, target, clauses) => {
            analyze_ast(target, warnings);

            // Run pattern analysis on these clauses (only if not an internal synthetic single-clause let match)
            let is_internal_let_destructure = clauses.len() == 1
                && clauses[0].guard.is_none()
                && !matches!(&clauses[0].pattern, Pattern::Wildcard(_) | Pattern::Variable(..));

            if !is_internal_let_destructure {
                let match_warnings = analyze_match(*loc, Some(target), clauses);
                warnings.extend(match_warnings);
            }

            for clause in clauses {
                if let Some(guard) = &clause.guard {
                    analyze_ast(guard, warnings);
                }
                for expr in &clause.body {
                    analyze_ast(expr, warnings);
                }
            }
        }
        Ast::TypeAssert(_, expr, _) => {
            analyze_ast(expr, warnings);
        }
        Ast::Implements(_, _, _, handler) => {
            analyze_ast(handler, warnings);
        }
        Ast::TypeSignature(..)
        | Ast::Newtype(..)
        | Ast::Derive(..)
        | Ast::Trait(..)
        | Ast::Import(..)
        | Ast::Bind(..)
        | Ast::Nil(..)
        | Ast::Symbol(..)
        | Ast::Integer(..)
        | Ast::Float(..)
        | Ast::String(..)
        | Ast::Boolean(..)
        | Ast::Char(..)
        | Ast::VisibilityDirective(..) => {}
    }
}

/// Recursively analyze a list of top-level ASTs and collect all compiler warnings.
pub fn analyze_program(asts: &[Ast]) -> Vec<SelWarning> {
    let mut warnings = Vec::new();
    for ast in asts {
        analyze_ast(ast, &mut warnings);
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::MatchClause;
    use crate::lexer::Loc;
    use crate::types::intern;

    #[test]
    fn test_unreachable_after_wildcard() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::Wildcard(loc),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Literal(loc, Box::new(Ast::Integer(loc, 42))),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], SelWarning::UnreachablePattern(..)));
    }

    #[test]
    fn test_exhaustive_booleans() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::Literal(loc, Box::new(Ast::Boolean(loc, true))),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Literal(loc, Box::new(Ast::Boolean(loc, false))),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert!(warnings.is_empty(), "Expected 0 warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_inexhaustive_boolean_missing_false() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::Literal(loc, Box::new(Ast::Boolean(loc, true))),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert_eq!(warnings.len(), 1);
        if let SelWarning::InexhaustivePattern(_, msg) = &warnings[0] {
            assert!(msg.contains("missing case: false"));
        } else {
            panic!("Expected InexhaustivePattern");
        }
    }

    #[test]
    fn test_exhaustive_lists() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::List(loc, vec![]),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Cons(
                    loc,
                    Box::new(Pattern::Wildcard(loc)),
                    Box::new(Pattern::Wildcard(loc)),
                ),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert!(warnings.is_empty(), "Expected 0 warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_unreachable_list_after_cons() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::List(loc, vec![]),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Cons(
                    loc,
                    Box::new(Pattern::Wildcard(loc)),
                    Box::new(Pattern::Wildcard(loc)),
                ),
                guard: None,
                body: vec![],
            },
            // [x] is already covered by [h | t]
            MatchClause {
                loc,
                pattern: Pattern::List(loc, vec![Pattern::Wildcard(loc)]),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], SelWarning::UnreachablePattern(..)));
    }

    #[test]
    fn test_or_pattern_reachability() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::Or(
                    loc,
                    vec![
                        Pattern::Literal(loc, Box::new(Ast::Integer(loc, 1))),
                        Pattern::Literal(loc, Box::new(Ast::Integer(loc, 2))),
                    ],
                ),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Literal(loc, Box::new(Ast::Integer(loc, 1))),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Wildcard(loc),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], SelWarning::UnreachablePattern(..)));
    }

    #[test]
    fn test_guarded_clause_does_not_shadow() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::Wildcard(loc),
                guard: Some(Ast::Boolean(loc, true)),
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Literal(loc, Box::new(Ast::Integer(loc, 42))),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Wildcard(loc),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert!(warnings.is_empty(), "Guarded clause must not make subsequent clauses unreachable");
    }

    #[test]
    fn test_multiclause_tuple_unpacking_exhaustive() {
        let loc = Loc::default();
        // target: (list arg0)
        let target = Ast::List(
            loc,
            vec![
                Ast::Symbol(loc, intern("list")),
                Ast::Symbol(loc, intern("arg0")),
            ],
        );
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::List(loc, vec![Pattern::List(loc, vec![])]), // []
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::List(
                    loc,
                    vec![Pattern::Cons(
                        loc,
                        Box::new(Pattern::Wildcard(loc)),
                        Box::new(Pattern::Wildcard(loc)),
                    )],
                ), // [h | t]
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, Some(&target), &clauses);
        assert!(warnings.is_empty(), "Expected 0 warnings for exhaustive unpacked multi-clause list matching, got: {:?}", warnings);
    }

    #[test]
    fn test_analyze_program_recursive() {
        let loc = Loc::default();
        let code = Ast::Define(
            loc,
            intern("foo"),
            Box::new(Ast::Lambda(
                loc,
                Some(intern("foo")),
                vec![intern("x")],
                vec![Ast::Match(
                    loc,
                    Box::new(Ast::Symbol(loc, intern("x"))),
                    vec![
                        MatchClause {
                            loc,
                            pattern: Pattern::Wildcard(loc),
                            guard: None,
                            body: vec![Ast::Integer(loc, 1)],
                        },
                        MatchClause {
                            loc,
                            pattern: Pattern::Literal(loc, Box::new(Ast::Integer(loc, 10))),
                            guard: None,
                            body: vec![Ast::Integer(loc, 2)],
                        },
                    ],
                )],
            )),
        );

        let warnings = analyze_program(&[code]);
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], SelWarning::UnreachablePattern(..)));
    }

    #[test]
    fn test_wildcard_after_list_not_unreachable() {
        let loc = Loc::default();
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::List(loc, vec![]),
                guard: None,
                body: vec![],
            },
            MatchClause {
                loc,
                pattern: Pattern::Cons(
                    loc,
                    Box::new(Pattern::Wildcard(loc)),
                    Box::new(Pattern::Wildcard(loc)),
                ),
                guard: None,
                body: vec![],
            },
            // In dynamic typing, _ matches non-lists (like integers or strings)
            MatchClause {
                loc,
                pattern: Pattern::Wildcard(loc),
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, None, &clauses);
        assert!(warnings.is_empty(), "Wildcard fallback in dynamic typing must remain reachable for non-list values, got: {:?}", warnings);
    }

    #[test]
    fn test_multiclause_tuple_inexhaustive() {
        let loc = Loc::default();
        let target = Ast::List(
            loc,
            vec![
                Ast::Symbol(loc, intern("list")),
                Ast::Symbol(loc, intern("arg0")),
            ],
        );
        let clauses = vec![
            MatchClause {
                loc,
                pattern: Pattern::List(loc, vec![Pattern::List(loc, vec![])]), // [] only
                guard: None,
                body: vec![],
            },
        ];

        let warnings = analyze_match(loc, Some(&target), &clauses);
        assert_eq!(warnings.len(), 1);
        assert!(matches!(warnings[0], SelWarning::InexhaustivePattern(..)));
    }
}
