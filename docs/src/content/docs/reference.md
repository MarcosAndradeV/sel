---
title: Language Reference
description: Comprehensive reference for the syntax, grammar, special forms, FFI rules, and execution models of the sel Lisp language.
---

`sel` is a Scheme-like Lisp dialect designed to be extremely lightweight, fast, and seamlessly integrated with native C libraries. This document defines the core language syntax, primitive types, special forms, compile-time metaprogramming constructs, error handling, coroutines, and the Foreign Function Interface (FFI).

---

## 1. Literals & Data Types

`sel` supports standard Lisp datatypes with modernized additions like curly-brace records and multiple integer bases.

### Nil
The empty value is represented by the keyword `nil`. It represents falsiness in logical assertions.
```lisp
nil
```

### Booleans
Represented by `#t` (true) and `#f` (false).
```lisp
#t
#f
```

### Integers & Numeric Bases
Integers are parsed as 64-bit signed values by default. `sel` natively supports writing literals in multiple bases:
- **Decimal**: `42` or `-100`
- **Hexadecimal**: `0xFF` (evaluates to `255`) or `0x1a` (evaluates to `26`)
- **Binary**: `0b101101` (evaluates to `45`)
- **Octal**: `0o755` (evaluates to `493`)

### Floats
Parsed as 64-bit floating point numbers:
```lisp
3.14159
-0.007
```

### Strings
Strings are UTF-8 encoded and enclosed in double quotes:
```lisp
"Hello, Lisp!"
```

### Characters
Characters are first-class values in `sel`. They are written using the `#\` prefix:
- **Single character**: `#\a`, `#\A`, `#\ `, `#\\`, `#\(`
- **Special character names**:
  - `#\space` (evaluates to `' '`)
  - `#\newline` (evaluates to `'\n'`)
  - `#\tab` (evaluates to `'\t'`)
  - `#\return` (evaluates to `'\r'`)

Character values can be compared using `eq?`, converted using `char->integer` and `integer->char`, and queried via the `char?` predicate.


### Symbols
Symbols are unique identifiers representing names of variables, functions, or keys. They are interned by the runtime for fast lookup:
```lisp
'my-variable
'name
```

### Lists
Lists are ordered sequences of values, constructed using parentheses `()` and separated by whitespace:
```lisp
(1 "two" #t)
```

### Records
Records are key-value mappings similar to dictionaries or maps. They are denoted using curly braces `{}` and parsed as identifier-value pairs:
```lisp
{name "Marcos" age 30 permissions 0o755}
```

---

## 2. Special Forms (Syntactic Constructs)

Special forms are built-in constructs handled directly by the parser and compiler rather than evaluated as standard function applications.

### define
Binds a value to a symbol globally or within the current scope.
```lisp
;; Variable definition
(define x 100)

;; Function definition (shorthand for binding a lambda)
(define (square n)
  (* n n))
```

### set!
Mutates the value bound to a symbol in the nearest visible lexical scope.
```lisp
(define counter 0)
(set! counter (+ counter 1))
```

### let
Introduces a local lexical binding block. Bindings inside a `let` are evaluated in parallel (they cannot see each other during initialization).
```lisp
(let ((a 10)
      (b 20))
  (+ a b)) ; Evaluates to 30
```

### if
A standard conditional branching construct. Evaluates a condition. If the condition is truthy (not `nil` and not `#f`), evaluates the `then` branch; otherwise, evaluates the optional `else` branch.
```lisp
(if (> x 10)
    "Large"
    "Small")
```

### begin
Executes a sequence of expressions in order and returns the value of the final expression. Used primarily for grouping side effects.
```lisp
(begin
  (println "Writing log...")
  (file-system 'write "log.txt" "Operation complete")
  #t)
```

### try / catch
Structured error handling framework. If the expression inside the `try` block triggers a runtime error, execution jumps to the `catch` block where the error value is bound to the specified variable.
```lisp
(try
  (error "Something went wrong")
  (catch err
    (println "Caught error:" err)))
```

### match
High-performance structural pattern matching with zero allocation overhead for destructuring. Matches a target expression against a sequence of pattern clauses.

```lisp
(match target-expr
  (pattern [guard] body ...)
  ...)
```

#### Supported Pattern Types

| Pattern | Description | Example |
| :--- | :--- | :--- |
| **Literal** | Matches exact values (numbers, strings, booleans, characters, nil, quoted symbols) | `42`, `"hello"`, `#\a`, `'admin`, `nil`, `'()` |
| **Wildcard** | Matches any value without binding | `_` |
| **Variable** | Binds the matched value to a local variable | `x`, `user_id` |
| **List** | Matches fixed sequences | `(1 2 3)`, `(list a b c)`, `('() "empty")` |
| **Cons** | Deconstructs head and tail of a list | `(cons head tail)` |
| **Rest** | Prefix patterns with remaining elements captured | `(first second & rest)`, `(list a & rest)` |
| **Record** | Matches curly-brace records by keys (supports partial matching) | `{name n age 30}`, `{user {id uid}}` |
| **Or-pattern** | Matches if any alternative matches (alternatives cannot bind vars) | `(or 1 2 3)`, `(or "yes" "y" #t)` |

#### Guard Clauses
Clauses can specify an additional boolean condition using `(where cond)`, `(when cond)`, `:where cond`, or `:when cond`. Guard expressions can reference variables bound by the pattern:

```lisp
(define (classify-number n)
  (match n
    (0 "zero")
    (n (where (< n 0)) "negative")
    (n :where (< n 100) "small-positive")
    (_ "large-positive")))
```

If a guard evaluates to `#f` or `nil`, pattern matching falls through to subsequent clauses.

#### Exhaustiveness & TCO
If no clause matches the value, a descriptive runtime error is raised: `(error "No matching pattern for value:" val)`. Clause bodies are executed sequentially with implicit `begin`, and the final expression in each clause preserves full Tail Call Optimization (TCO).

```lisp
;; Recursive list traversal with TCO
(define (sum-list l acc)
  (match l
    ('() acc)
    ((cons h t) (sum-list t (+ acc h)))))
```

### defn
Defines a named function using multi-clause structural pattern matching, inspired by Elixir and Erlang. Each clause specifies a pattern to match against the arguments, with optional `:where` or `:when` guards and an optional `:do` marker.

```lisp
;; Single-argument function with shorthand name:
(defn fib
  (0 :do 0)
  (1 :do 1)
  (n :do (+ (fib (- n 1)) (fib (- n 2)))))

;; Multi-argument function with guards:
(defn (discount tier amount)
  (('vip amt) :where (>= amt 1000) :do 0.25)
  (('vip _) :do 0.15)
  (('member amt) :where (>= amt 500) :do 0.10)
  ((_ _) :do 0.0))

;; Destructuring record arguments:
(defn (handle-event evt)
  ({type 'login user u} :where (eq? u "admin") :do "admin-session")
  ({type 'login user u} :do "user-session")
  (_ :do "ignored"))
```

### with
Railway-oriented validation and execution construct inspired by Elixir's `with`. Each step matches an evaluated expression against a pattern. If all steps match, the `:do` block executes. If any step fails to match, execution short-circuits immediately. When an `:else` block is provided, the mismatched value is dispatched to the matching `:else` pattern; without `:else`, the non-matching value is returned directly:

```lisp
(with (((list 'ok user) (fetch-user id))
       ((list 'ok perms) (fetch-permissions (rget user 'role))))
  :do
  (render-dashboard user perms)
  :else
  ((list 'error 'user-not-found) :do "User missing from system")
  ((list 'error 'invalid-role) :do "Assigned role is invalid")
  (other :do "Unexpected error occurred"))
```

### for
List comprehensions over one or more collection generators, supporting intermediate `:let` bindings, conditional filtering with `:when` or `:where`, and structural pattern matching on generator items:

```lisp
;; Filtered transformation:
(for (x (range 10))
  :when (= (mod x 2) 0)
  :do (* x x))
;; => (0 4 16 36 64)

;; Multi-generator cartesian product with local bindings:
(for (x '(1 2 3))
     (y '(10 20))
  :let ((sum (+ x y)))
  :when (> sum 15)
  :do sum)
;; => (21 22 23)

;; Pattern matching on records (automatically filters non-matching items):
(for ({name n role 'admin} users)
  :do n)
```

### load
Loads and evaluates external Scheme files dynamically in the current lexical environment.

Unlike `import`, which loads code inside a prefixed namespace (e.g. `point/new-point`), `(load <path-expr>)` evaluates the target file directly in the caller's active environment. Any bindings or macros defined inside the loaded script will reside directly in the caller's environment.

`load` returns the value of the last evaluated expression in the loaded file.

```lisp
;; Load variables and functions from a helper file
(define res (load "helpers/math_utils.scm"))

;; Use a loaded function directly (no prefix namespace needed)
(println (square 10))

;; Check the return value of load (value of last expression in file)
(println "Loaded helpers returned:" res)
```

### import
Loads external Scheme library files into the current runtime environment.

`sel` supports module namespaces by prefixing exported names with the module name (e.g. `point/new-point`). You can customize this prefix using the `:as` keyword or a nested shorthand structure:

```lisp
;; Standard import (prefix matches module name: 'point')
(import point)
(point/new-point 1 2)

;; Inline alias using :as
(import point :as p)
(p/new-point 3 4)

;; Nested list with :as
(import (point :as pt))
(pt/new-point 5 6)

;; Nested list shorthand
(import (point pnt))
(pnt/new-point 7 8)
```

#### Module Visibility & Directives (`:public` / `:private`)
When defining a module in a separate file (e.g., `math.scm`), all top-level bindings are **public** and exported by default. You can control visibility using module directives:
- **`:private`**: Switches subsequent bindings to private scope. They can only be accessed internally within the module file.
- **`:public`**: Switches subsequent bindings back to public scope (exported).

Example module (`mod_example.scm`):
```lisp
;; mod_example.scm
(define a 10) ; Public by default

:private
(define b 20) ; Private (not exported)
(define c 30) ; Private (not exported)

:public
(define d 40) ; Public again
(define f 50) ; Public again
```

When importing this module:
```lisp
(import mod_example)

(println mod_example/a) ; Prints 10
(println mod_example/d) ; Prints 40

;; Accessing private bindings throws a runtime error:
mod_example/b ; Error: Undefined/private variable mod_example/b
```

---

## 3. Functions & Macros

### lambda
Constructs an anonymous, first-class function (closure) capturing the surrounding lexical scope.
```lisp
(define add-one (lambda (x) (+ x 1)))
```

#### Rest & Variadic Arguments (`&`)
You can bind remaining arguments to a list using the ampersand `&` prefix inside the parameter list:
```lisp
(define (sum-all &xs)
  (foldl + 0 xs))
```

### Lambda Shorthand (`\`)
A backslash `\` acts as a syntactic shortcut for lambda definitions, making high-order functional applications extremely compact:
```lisp
;; Standard:
(map (lambda (x) (* x 2)) '(1 2 3))

;; Shorthand:
(map \(x) (* x 2) '(1 2 3))
```

### defmacro
Defines a compile-time macro. Macros receive unevaluated expressions as arguments, compile them into a new AST template, and return that template for subsequent compilation and execution.
```lisp
(defmacro (unless condition body)
  (list 'if condition 'nil body))
```

---

## 4. Quasiquotation & Metaprogramming

Macros rely extensively on quoting structures to manipulate syntax safely:

- **Quote (`'`)**: Prevents evaluation of an S-expression.
- **Quasiquote (`` ` ``)**: Template quote. Allows sub-expressions to be evaluated and injected into the template.
- **Unquote (`~`)**: Evaluates a sub-expression inside a quasiquote.
- **Unquote-Splicing (`~@`)**: Evaluates a list and splices its contents directly into the parent list structure.

```lisp
(define name "World")
(define items '(a b))

;; Quasiquotation injection
`("Hello" ~name "Goodbye" ~@items) 
;; Evaluates to: ("Hello" "World" "Goodbye" a b)
```

---

## 5. Pipelines (`|>` and `->`)

`sel` provides two complementary pipeline operators to chain transformations sequentially without deep nesting:

### Pipeline Operator (`|>`) - Thread-Last (Collections)
`|>` rewrites subsequent expressions to inject the previous result as the **last** argument of each step. This is ideal for collection transformations (`map`, `filter`, `foldl`, etc.):

```lisp
;; Without pipelines:
(reverse (filter even? (map (lambda (x) (* x 2)) (range 5))))

;; With |> (thread-last):
(|> (range 5)
    (map (lambda (x) (* x 2)))
    (filter even?)
    (reverse))
;; => (8 6 4 2 0)
```

### Thread-First Operator (`->`) - Thread-First (Records & Objects)
`->` rewrites subsequent expressions to inject the previous result as the **first** argument of each step (immediately following the function name). This is ideal for records and dictionaries:

```lisp
;; Threading record transformations:
(-> %{}
    (assoc 'name "Alice")
    (assoc 'age 30)
    (assoc 'role "Engineer")
    (dissoc 'role))
;; => %{age: 30, name: "Alice"}
```

---

## 6. Native Coroutines

Coroutines are cooperative multi-tasking blocks that can suspend execution, return intermediate values, and resume from where they left off.

- `(co-create <closure>)`: Creates a coroutine from a zero-argument function. State is initially `'suspended`.
- `(co-resume <coroutine> <value>)`: Resumes the coroutine, optionally passing a value into the yield point.
- `(co-yield <value>)`: Suspends execution of the current coroutine and returns the value to the resumer.
- `(co-state <coroutine>)`: Queries the current state of the coroutine (`'suspended`, `'running`, or `'dead`).
- `(co-dead? <coroutine>)`: Evaluates to `#t` if the coroutine has finished executing its body.

### Coroutine Example
```lisp
(define generator 
  (co-create (lambda ()
    (println "Step 1")
    (co-yield 10)
    (println "Step 2")
    (co-yield 20)
    "Finished!")))

(println (co-resume generator nil)) ; Prints "Step 1", returns 10
(println (co-resume generator nil)) ; Prints "Step 2", returns 20
(println (co-resume generator nil)) ; Returns "Finished!"
```

---

## 7. Foreign Function Interface (FFI)

`sel` features an incredibly powerful, zero-boilerplate dynamic Foreign Function Interface (FFI) powered by `libffi` and `libloading`. You can load shared binary libraries directly and run functions natively.

### Core FFI Primitives

- `(ffi-dlopen <lib-path>)`: Opens a shared object file (`.so`, `.dylib`, or `.dll`) and returns a raw library pointer.
- `(ffi-dlsym <lib-pointer> <symbol-name>)`: Searches the loaded library for a symbol by name and returns a symbol pointer.
- `(ffi-call <symbol-pointer> <return-type> (<arg-types>) <args...>)`: Executes the binary function.

### FFI Types & Coercions

| Type Selector | C Type Mapping | Lisp Behavior / Type Coercion |
| :--- | :--- | :--- |
| `'void` | `void` | Used only as return type. Returns `nil` in `sel`. |
| `'bool` | `bool` / `u8` | Maps to `Value::Boolean`. |
| `'u8`, `'uchar` | `uint8_t`, `unsigned char` | Maps Lisp integers to 8-bit unsigned integer structures. (Also accepts character literals `#\a` via automatic coercion). |
| `'i8`, `'ichar` | `int8_t`, `signed char` | Maps Lisp integers to 8-bit signed integer structures. (Also accepts character literals `#\a` via automatic coercion). |
| `'char` | `char` | Maps to first-class Lisp character (`char`) values directly. |
| `'u16` | `uint16_t`, `unsigned short` | Maps Lisp integers to 16-bit unsigned integer structures. |
| `'i16` | `int16_t`, `short` | Maps Lisp integers to 16-bit signed integer structures. |
| `'i32`, `'u32` | `int32_t`, `uint32_t` | Evaluates Lisp numbers to 32-bit width integer structures. |
| `'i64`, `'u64` | `int64_t`, `uint64_t` | Maps to Lisp integers natively. |
| `'f32`, `'f64` | `float`, `double` | Evaluates Lisp floats natively. |
| `'*u8` | `void*` or `char*` | If passed a string, `sel` automatically allocates a null-terminated C-string array and manages its memory lifecycle for the duration of the call. |
| `'(struct (t1 t2 ...))` | `struct` | Passed or returned by value. Represented in `sel` as nested lists matching the types of their fields. Padded automatically at runtime. |

### Passing and Returning C Structures (by Value)

The FFI supports passing and returning C structures directly by value using the `'(struct (<field-types>))` selector.

#### Representation
C structures are mapped to `sel` lists or nested lists. Field offsets and padding alignments are calculated automatically according to the System V ABI standards:
- A flat C `struct { float x; float y; }` maps to a list: `'(x_val y_val)`
- A nested C `struct { struct Point pos; float w; float h; }` maps to a nested list: `'((x_val y_val) w_val h_val)`

#### Struct FFI Example
```lisp
(define lib (ffi-dlopen "./libffi_test_structs.so"))

;; 1. Pass a struct by value
(define get-distance-sq (ffi-func (ffi-dlsym lib "get_distance_sq") 'f32 '((struct (f32 f32)))))
(define d (get-distance-sq '(3.0 4.0))) ; Evaluates to 25.0

;; 2. Return a struct by value
(define make-point (ffi-func (ffi-dlsym lib "make_point") '(struct (f32 f32)) '(f32 f32)))
(define p (make-point 5.0 12.0)) ; Evaluates to '(5.0 12.0)
(assert (eq? (car p) 5.0))

;; 3. Pass nested structures by value
(define get-rect-area (ffi-func (ffi-dlsym lib "get_rect_area") 'f32 '((struct ((struct (f32 f32)) f32 f32)))))
(define area (get-rect-area '((10.0 20.0) 5.0 8.0))) ; Evaluates to 40.0
```

### Complete FFI Showcase
```lisp
;; Load libc functions
(define libc (ffi-dlopen "libc.so.6"))
(define strlen (ffi-dlsym libc "strlen"))
(define puts (ffi-dlsym libc "puts"))

;; Invoke strlen
(print (ffi-call strlen 'u64 '(*u8) "Hello, FFI!")) ; Evaluates to 11

;; Invoke puts
(ffi-call puts 'i32 '(*u8) "Printed directly by C!")

;; Math functions from libm
(define libm (ffi-dlopen "libm.so.6"))
(define my_pow (ffi-dlsym libm "pow"))
(print (ffi-call my_pow 'f64 '(f64 f64) 2.0 3.0)) ; Evaluates to 8.0
```
