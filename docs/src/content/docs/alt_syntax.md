---
title: Modern Functional Syntax (.sel)
description: Complete guide to SEL's modern functional syntax, pipelines, multi-clause pattern matching, and dual-syntax ecosystem.
---

SEL features a dual-syntax architecture:
1. **Classic Scheme/Lisp S-expressions (`.scm`)**: Minimalist, macro-heavy, homoiconic syntax.
2. **Modern Functional Syntax (`.sel`)**: Clean, indentation-friendly, expression-oriented syntax inspired by Unison, Elixir, and ML, featuring thread pipelines, first-class pattern matching, multi-clause functions, and records.

Both syntaxes compile to the same underlying Abstract Syntax Tree and bytecode runtime, sharing environment namespaces and modules seamlessly.

---

## 1. Syntax Basics & Definitions

### Variables and Functions
Variables and functions use the `:=` assignment operator:

```sel
// Variable definition
x := 42
pi := 3.14159
name := "SEL"

// Function definition
add a b := a + b
square n := n * n

// Invocation uses standard parentheses
total := add(10, square(5)) // 35
```

### Module Visibility (`pub`)
Top-level declarations are private to the file by default. Export symbols using `pub`:

```sel
pub greet person := "Hello, " + person
```

### Local Scopes: `let ... in ...` and `do ... end`
```sel
// Let binding
hypotenuse a b :=
    let a2 = a * a,
        b2 = b * b
    in sqrt(a2 + b2)

// Multi-statement imperative-style block
compute :=
    do
        step1 := expensive_calc()
        step2 := step1 * 2
        step2 + 10
    end
```

---

## 2. Multi-Clause Pattern-Matching Functions

Functions can be defined with multiple clauses that match on parameter patterns:

```sel
// Fibonacci with base cases
fib 0 := 0
fib 1 := 1
fib n := fib(n - 1) + fib(n - 2)

// List processing with cons pattern matching [head | tail]
sum [] := 0
sum [h | t] := h + sum(t)
```

### Pattern Guards (`when`)
Clauses can include guards to further constrain matching:

```sel
fact n when n <= 1 := 1
fact n := n * fact(n - 1)

classify temp when temp < 0  := :freezing
classify temp when temp < 20 := :cool
classify temp when temp < 30 := :warm
classify _                   := :hot
```

> **Rules**:
> - All clauses for a given function must be contiguous. Intervening statements will trigger a compile error.
> - The `pub` modifier may only be specified on the first clause.
> - All clauses must have identical arity.

---

## 3. Data Structures: Lists & Records

### Lists and Cons Syntax
Lists use bracket notation `[...]`. In both expressions and pattern matching, the cons separator `|` allows head/tail manipulation:

```sel
// Creating lists
empty := []
nums := [1, 2, 3, 4]
cons_list := [1 | [2, 3, 4]] // [1, 2, 3, 4]
prepended := [0, 1 | [2, 3]] // [0, 1, 2, 3]

// Pattern matching destructuring
first_two [a, b | _] := [a, b]
first_two _ := []
```

### Records & Dot-Access
Records are immutable key-value maps with concise curly-brace syntax:

```sel
user := { id: 101, name: "Alice", active: true }

// Dot-access
println(user.name) // "Alice"
println(user.id)   // 101
```

---

## 4. Pipeline Operators: Thread-First & Thread-Last

Pipelines streamline nested function calls into linear dataflow transformations:

### Thread-First (`|>`)
Inserts the left-hand expression as the **first argument** of the right-hand call:
`x |> f(y, z)` desugars to `f(x, y, z)`.

```sel
nums := [1, 2, 3, 4, 5]

result := nums
    |> filter_by(\x -> is_even(x))
    |> map_by(\x -> x * 10)
    |> sum
// result == 60
```

### Thread-Last (`|>>`)
Inserts the left-hand expression as the **last argument** of the right-hand call:
`x |>> f(y, z)` desugars to `f(y, z, x)`.

```sel
sub a b := a - b

35 |> sub(10)   // sub(35, 10) = 25
10 |>> sub(35)  // sub(35, 10) = 25
```

---

## 5. Pattern Matching: `match ... with ... end`

Full pattern matching is available anywhere within expressions:

```sel
grade score :=
    match score with
    | s when s >= 90 -> :a
    | s when s >= 80 -> :b
    | _ -> :c
    end

// Record pattern matching
handle_response resp :=
    match resp with
    | { status: 200, data: d } -> d
    | { status: code } when code >= 500 -> :server_error
    | _ -> :unknown
    end
```

---

## 6. Standard Library Prelude (`core.sel`)

When running with alternative syntax enabled, SEL automatically loads `src/core.sel` alongside the Scheme core prelude.

### Type Predicates & Introspection
- `type_of(x)`: returns type symbol (`:int`, `:float`, `:string`, `:list`, `:record`, etc.)
- `is_int(x)`, `is_float(x)`, `is_bool(x)`
- `is_string(x)`, `is_list(x)`, `is_record(x)`, `is_function(x)`
- `is_even(x)`, `is_odd(x)`
- `to_string(x)`, `to_int(x)`, `to_float(x)`

### List & Collection Helpers
- `first(list)` / `head(list)`: gets the first element
- `rest(list)` / `tail(list)`: gets the tail of the list
- `take(list, n)` / `take(n, list)`: takes first `n` elements (works on lists & strings)
- `drop(list, n)` / `drop(n, list)`: drops first `n` elements (works on lists & strings)
- `sum(list)`: sums all numeric elements
- `product(list)`: multiplies all numeric elements
- `contains(list, item)`: checks if item exists in list
- `find(list, pred)`: returns first matching item, or `nil`
- `any(list, pred)`: checks if any element satisfies predicate
- `all(list, pred)`: checks if all elements satisfy predicate

### Pipeline Higher-Order Functions
- `map_by(list, fn)`: transforms each element using `fn`
- `filter_by(list, pred)`: filters elements by `pred`
- `reduce(list, acc, fn)`: folds list using `fn` starting with `acc`

---

## 7. Dual-Syntax Interoperability

SEL allows seamless interop between Scheme (`.scm`) and Modern (`.sel`) modules.

### Escaped Scheme Identifiers (`:'<name>'`)
Scheme identifiers often contain hyphens or question marks (e.g. `type-of`, `nil?`, `empty?`). You can refer to or define hyphenated symbols in `.sel` using `:'...'`:

```sel
// Calling Scheme functions directly:
println(:'type-of'(42)) // :int
println(:'nil?'(nil))   // true

// Importing a Scheme module in .sel:
import point as pt
p := pt.new_point(3, 4)
println(p.x) // 3
```

### Embedding in Rust
When embedding `sel` in Rust applications, use `eval_alt` or `eval_alt_sandboxed`:

```rust
use sel::{eval_alt, eval_alt_sandboxed, Env, load_core_lib};
use std::rc::Rc;
use std::cell::RefCell;

let env = Rc::new(RefCell::new(Env::default()));
env.borrow_mut().parent = Some(load_core_lib());

let result = eval_alt("x := 10\nx * 2", env).unwrap();
assert_eq!(format!("{result}"), "20");
```
