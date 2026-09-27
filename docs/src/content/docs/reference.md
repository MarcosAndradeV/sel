---
title: Language Reference
description: Comprehensive reference for the syntax, grammar, data types, execution model, coroutines, FFI, and metaprogramming in SEL.
---

`SEL` is a modern, indentation-friendly, expression-oriented functional programming language implemented in Rust. It combines functional purity, pattern matching, pipelines, immutable records, cooperative coroutines, and seamless C Foreign Function Interface (FFI) integration.

---

## 1. Literals & Data Types

### Nil
The absence of a value is represented by `nil`:
```sel
nil
```

### Booleans
Boolean truth values are written `true` and `false`:
```sel
is_ready := true
is_done := false
```

### Integers & Numeric Bases
Integers are 64-bit signed values by default (`i64`). SEL supports literals in multiple bases:
- **Decimal**: `42`, `-100`
- **Hexadecimal**: `0xFF` (evaluates to `255`), `0x1a` (evaluates to `26`)
- **Binary**: `0b101101` (evaluates to `45`)
- **Octal**: `0o755` (evaluates to `493`)

### Floats
64-bit IEEE 754 floating point numbers:
```sel
pi := 3.14159
rate := -0.007
```

### Strings
UTF-8 encoded strings enclosed in double quotes:
```sel
greeting := "Hello, SEL!"
```

### Characters
First-class Unicode scalar values written with the `#\` prefix:
- **Single character**: `#\a`, `#\Z`, `#\0`
- **Special named characters**:
  - `#\space`
  - `#\newline`
  - `#\tab`
  - `#\return`

Conversions between characters and integer scalar values are performed via `char_to_integer` and `integer_to_char`:
```sel
code := char_to_integer(#\a) // 97
ch := integer_to_char(97)     // #\a
```

### Atoms & Symbols
Atoms are lightweight, interned constants prefixed with a colon `:`:
```sel
status := :ok
err := :not_found
```
Quoted symbols can also be written `'sym`.

### Lists
Lists are immutable ordered sequences enclosed in brackets `[...]`:
```sel
empty := []
nums := [1, 2, 3, 4]
mixed := [1, "two", :three, true]
```
The cons operator `|` deconstructs or constructs head and tail elements:
```sel
prepended := [0 | nums]        // [0, 1, 2, 3, 4]
multi_prep := [10, 20 | nums]  // [10, 20, 1, 2, 3, 4]
```

### Records
Records are immutable associative key-value mappings denoted with curly braces `{}`:
```sel
user := { id: 101, name: "Alice", active: true }

// Dot-access
println(user.name) // "Alice"
println(user.id)   // 101
```

---

## 2. Variables, Functions & Definitions

### Definitions (`:=`)
Variables and functions are declared using `:=`:

```sel
// Variable definition
x := 42
name := "SEL"

// Function definition
add a b := a + b
square n := n * n

// Function invocation
result := add(10, square(5)) // 35
```

### Module Visibility (`pub`)
Top-level declarations are private to the file by default. Use `pub` to export definitions:

```sel
pub greet person := "Hello, " + person
```

### Anonymous Functions (Lambdas)
Lambdas are defined using `\args -> body`:

```sel
double := \x -> x * 2
multiply := \x y -> x * y

// Passing to higher-order functions
nums := [1, 2, 3] |> map_by(\x -> x * 10) // [10, 20, 30]
```

### Local Scopes & Destructuring (`let ... in ...`)
`let` introduces local lexical bindings and supports full pattern destructuring across lists, records, and as-patterns:

```sel
// Simple binding
hypotenuse a b :=
    let a2 = a * a,
        b2 = b * b
    in sqrt(a2 + b2)

// Destructuring lists
coordinates := [10, 20]
dist_sq := let [x, y] = coordinates in x * x + y * y

// Destructuring records with field punning
user := { id: 101, name: "Alice", role: :admin }
greeting := let { name, role } = user in format("{} is {}", name, role)

// Destructuring with As-patterns
summary := let all @ [h | t] = [1, 2, 3] in { head: h, count: count(all), rest: t }
```

If a destructuring pattern fails to match the provided value at runtime, a descriptive error is raised.

### Imperative Blocks (`do ... end`)
`do ... end` executes a sequence of expressions and yields the value of the final expression:

```sel
compute :=
    do
        step1 := expensive_calc()
        step2 := step1 * 2
        step2 + 10
    end
```

---

## 3. Multi-Clause Pattern-Matching Functions

Functions can be defined across multiple contiguous clauses matching argument patterns:

```sel
// Fibonacci with pattern matching base cases
fib 0 := 0
fib 1 := 1
fib n := fib(n - 1) + fib(n - 2)

// List processing with cons patterns
sum [] := 0
sum [h | t] := h + sum(t)
```

### Pattern Guards (`when`)
Clauses can specify boolean condition guards with `when`:

```sel
fact n when n <= 1 := 1
fact n := n * fact(n - 1)

classify temp when temp < 0  := :freezing
classify temp when temp < 20 := :cool
classify temp when temp < 30 := :warm
classify _                   := :hot
```

> **Rules for Multi-Clause Functions**:
> - All clauses for a given function name must appear contiguously.
> - The `pub` modifier may only be attached to the first clause.
> - All clauses must have identical arity.

---

## 4. Pattern Matching Expressions (`match ... with`)

Pattern matching can be evaluated anywhere inside expressions using `match ... with ... end`:

```sel
grade score :=
    match score with
    | s when s >= 90 -> :a
    | s when s >= 80 -> :b
    | _ -> :c
    end
```

### Supported Patterns

| Pattern Kind | Syntax | Description |
| :--- | :--- | :--- |
| **Wildcard** | `_` | Matches any value without binding |
| **Variable** | `x`, `id` | Binds matched value to variable name |
| **Literal** | `0`, `"hello"`, `:ok` | Matches exact constant values |
| **As-Pattern** | `var @ pat` | Binds `var` to the whole value while matching `pat` |
| **Or-Pattern** | `(p1 \| p2 \| ...)` | Matches if any alternative matches (no bindings) |
| **Cons** | `[h \| t]` | Deconstructs head and tail of list |
| **List** | `[a, b, c]`, `[]` | Matches exact list structure |
| **Rest** | `[a, b \| rest]` | Matches prefix elements and binds remaining tail |
| **Record** | `{ id: uid, status: s }` | Matches record fields by key (with punning `{ key }`) |

### As-Patterns (`@`)
Capture the composite container while matching and destructuring inner components:

```sel
inspect_list all @ [] := "Empty"
inspect_list all @ [x] := format("Single: {}", x)
inspect_list all @ [h | t] :=
    format("Total {} items, head is {}", count(all), h)
```

### Parenthesized Or-Patterns (`(p1 | p2 | ...)`)
Group multiple alternative literal patterns cleanly without duplicating handler bodies:

```sel
status_category (200 | 201 | 204) := :success
status_category (400 | 401 | 404) := :client_error
status_category (500 | 502 | 503) := :server_error
status_category _ := :other

// Nested or-patterns in lists or records
match event with
| [:http, (200 | 204)] -> "OK"
| { role: (:admin | :owner) } -> "Authorized"
| _ -> "Other"
end
```

### Record Pattern Matching
```sel
handle_response resp :=
    match resp with
    | { status: 200, data: d } -> d
    | { status: code } when code >= 500 -> :server_error
    | _ -> :unknown
    end
```

---

## 5. Control Flow & Operators

### Conditionals (`if ... then ... else`)
Conditionals are expressions returning a value:

```sel
message := if score >= 60 then "Passed" else "Failed"
```

### Operators
- **Arithmetic**: `+`, `-`, `*`, `/`, `%` / `mod`
- **Relational**: `<`, `<=`, `>`, `>=`
- **Equality**: `==` (structural equality), `!=` (inequality)
- **Logical**: `!` / `not`, `&&`, `||`

---

## 6. Pipeline Operators (`|>` and `|>>`)

SEL features two directional pipeline operators for clean data transformation chains:

### Thread-First (`|>`)
Injects the left-hand value as the **first argument** of the right-hand call:
`x |> f(y)` desugars to `f(x, y)`.

```sel
nums := [1, 2, 3, 4, 5]

result := nums
    |> filter_by(\x -> is_even(x))
    |> map_by(\x -> x * 10)
    |> sum
// result == 60
```

### Thread-Last (`|>>`)
Injects the left-hand value as the **last argument** of the right-hand call:
`x |>> f(y)` desugars to `f(y, x)`.

```sel
sub a b := a - b

35 |> sub(10)   // sub(35, 10) = 25
10 |>> sub(35)  // sub(35, 10) = 25
```

---

## 7. Error Handling

### Try / Catch Expressions
Runtime errors can be intercepted safely with `try ... catch`:

```sel
safe_div a b :=
    try
        a / b
    catch err ->
        :division_error
```

### Monadic Results
Standard prelude provides result constructors:
```sel
res1 := ok(42)
res2 := err("Connection timed out")

is_ok(res1)  // true
is_err(res2) // true
unwrap(res1) // 42
unwrap_or(res2, 0) // 0
```

---

## 8. Modules & Loading

### `import`
Imports an external SEL script into a prefixed module namespace:

```sel
// Imports definitions from 'point.sel' as 'point.<name>'
import point

p := point.new_point(10, 20)
println(p.x) // 10

// Import with alias
import point as pt
p2 := pt.new_point(30, 40)
```

Only definitions marked with `pub` are accessible from outside the module:
```sel
// math_module.sel
pub add a b := a + b
secret_salt := 12345 // private
```

### `load`
Loads and evaluates a script directly in the current scope without namespacing:
```sel
load("helpers.sel")
```

---

## 9. Native Coroutines

Coroutines provide cooperative multitasking with yield and resume primitives:

- `co_create(closure)`: Creates a suspended coroutine.
- `co_resume(coroutine, value)`: Resumes execution, passing `value`.
- `yield value`: Suspends current coroutine, returning `value` to caller.
- `co_state(coroutine)`: Returns `:suspended`, `:running`, or `:dead`.
- `co_dead(coroutine)`: Returns `true` if coroutine has finished.

### Example
```sel
generator := co_create(\ -> do
    println("Step 1")
    yield 10
    println("Step 2")
    yield 20
    "Finished!"
end)

println(co_resume(generator, nil)) // Prints "Step 1", returns 10
println(co_resume(generator, nil)) // Prints "Step 2", returns 20
println(co_resume(generator, nil)) // Returns "Finished!"
```

---

## 10. Foreign Function Interface (FFI)

SEL features zero-boilerplate dynamic FFI powered by `libffi` and `libloading`.

### Core FFI Primitives
- `ffi_dlopen(path)`: Opens a shared object file (`.so`, `.dylib`, or `.dll`).
- `ffi_dlsym(handle, symbol_name)`: Looks up a symbol pointer.
- `ffi_call(fn_ptr, return_type, arg_types, ...args)`: Calls native function.
- `ffi_func(fn_ptr, return_type, arg_types)`: Wraps a symbol in a callable SEL closure.

### C Type Selectors
`'void`, `'bool`, `'u8`, `'i8`, `'u16`, `'i16`, `'u32`, `'i32`, `'u64`, `'i64`, `'f32`, `'f64`, `'*u8` (C string), `'(struct (t1 t2 ...))`.

### FFI Example
```sel
libc := ffi_dlopen("libc.so.6")
puts := ffi_dlsym(libc, "puts")
strlen := ffi_dlsym(libc, "strlen")

ffi_call(puts, 'i32, ['*u8], "Printed directly from C!")
len := ffi_call(strlen, 'u64, ['*u8], "Hello, FFI!")
println("Length:", len)
```

---

## 11. Macros & Metaprogramming

SEL includes a macro system for syntactic transformations. Macros receive unevaluated AST forms and return expanded code for execution.

### `defmacro`
Defines a macro at compile-time:
```sel
(defmacro (unless condition body)
  (list 'if condition 'nil body))
```

### Quasiquotation
- **Quote (`'`)**: Suppresses evaluation.
- **Quasiquote (`` ` ``)**: Template quote allowing selective unquoting.
- **Unquote (`~`)**: Evaluates expression inside a quasiquote.
- **Unquote-Splicing (`~@`)**: Evaluates a list and splices contents into template.
- **`gensym(prefix)`**: Generates a globally unique identifier to prevent macro hygiene capture.

```sel
(define name "World")
(define items '(a b))

`("Hello" ~name "Goodbye" ~@items)
// ("Hello" "World" "Goodbye" a b)
```

---

## 12. Embedding SEL in Rust

SEL can be embedded in Rust applications via the standard library interface:

```rust
use sel::{eval, eval_sandboxed, Env, load_core_lib};
use std::rc::Rc;
use std::cell::RefCell;

let env = Rc::new(RefCell::new(Env::default()));
env.borrow_mut().parent = Some(load_core_lib());

let result = eval("x := 10\nx * 2", env).unwrap();
assert_eq!(format!("{result}"), "20");
```
