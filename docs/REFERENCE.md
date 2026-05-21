# Language Reference

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

## 5. Pipelines & Thread-First (`->`)

`sel` includes a native thread-first operator (`->`) which rewrites subsequent expressions to inject the previous result as the **last** argument. This allows deeply nested function calls to be read as sequential processing steps.

```lisp
;; Without thread-first:
(reverse (filter even? (range 10)))

;; With thread-first:
(-> (range 10)
    (filter even?)
    (reverse))
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
| `'i32`, `'u32` | `int32_t`, `uint32_t` | Evaluates Lisp numbers to 32-bit width integer structures. |
| `'i64`, `'u64` | `int64_t`, `uint64_t` | Maps to Lisp integers natively. |
| `'f32`, `'f64` | `float`, `double` | Evaluates Lisp floats natively. |
| `'*u8` | `void*` or `char*` | If passed a string, `sel` automatically allocates a null-terminated C-string array and manages its memory lifecycle for the duration of the call. |
| `'(struct (t1 t2 ...))` | `struct` | Passed or returned by value. Represented in `sel` as nested lists matching the types of their fields. Padded automatically at runtime. |

### Passing and Returning C Structures (by Value)

Starting in `v0.1.1`, the FFI supports passing and returning C structures directly by value using the `'(struct (<field-types>))` selector.

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
