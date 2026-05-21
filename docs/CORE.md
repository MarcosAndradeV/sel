# Standard & Core Library Reference

This document describes all functions, macros, and operators available globally in the `sel` Lisp runtime environment. They are divided into **Native Built-in Functions** (implemented directly in the Rust interpreter core) and the **Scheme Standard Library** (defined in `core.scm`).

---

## 1. Native Built-in Functions (Rust-Implemented)

These core primitives are registered directly inside the environment by the interpreter runtime.

### Arithmetic & Numeric Operators

| Function | Arity | Description | Example |
| :--- | :--- | :--- | :--- |
| `+` | Variadic | Sums all numeric arguments. | `(+ 1 2 3)` $\rightarrow$ `6` |
| `-` | $\ge 1$ | Subtraction. If 1 argument is passed, returns its negation. | `(- 10 3)` $\rightarrow$ `7`, `(- 5)` $\rightarrow$ `-5` |
| `*` | Variadic | Multiplies all numeric arguments. | `(* 2 3 4)` $\rightarrow$ `24` |
| `/` | $\ge 2$ | Performs division. Supports integers and floats. | `(/ 10 2)` $\rightarrow$ `5`, `(/ 5.0 2.0)` $\rightarrow$ `2.5` |
| `mod` | 2 | Computes the remainder of dividing the first argument by the second. | `(mod 10 3)` $\rightarrow$ `1` |

### Equalities & Comparators

| Function | Arity | Description | Example |
| :--- | :--- | :--- | :--- |
| `eq?` | 2 | Checks identity equality. Evaluates symbols, nil, booleans, and numbers. | `(eq? 'a 'a)` $\rightarrow$ `#t`, `(eq? 1 1)` $\rightarrow$ `#t` |
| `=` | 2 | Numeric value equality check. | `(= 5 5.0)` $\rightarrow$ `#t` |
| `!=` | 2 | Numeric value inequality check. | `(!= 5 10)` $\rightarrow$ `#t` |
| `<` | 2 | Evaluates if the first number is strictly less than the second. | `(< 2 3)` $\rightarrow$ `#t` |
| `>` | 2 | Evaluates if the first number is strictly greater than the second. | `(> 5 3)` $\rightarrow$ `#t` |
| `<=` | 2 | Evaluates if the first number is less than or equal to the second. | `(<= 3 3)` $\rightarrow$ `#t` |
| `>=` | 2 | Evaluates if the first number is greater than or equal to the second. | `(>= 5 2)` $\rightarrow$ `#t` |
| `not` | 1 | Logically negates the boolean argument. | `(not #f)` $\rightarrow$ `#t`, `(not nil)` $\rightarrow$ `#t` |

### Type Query & Reflection

These predicates allow runtime type introspection. They all take **1 argument** and return a boolean (`#t` or `#f`).

- `(nil? x)`: Returns `#t` if `x` is `nil`.
- `(list? x)`: Returns `#t` if `x` is a sequence list.
- `(number? x)`: Returns `#t` if `x` is an integer or floating-point number.
- `(string? x)`: Returns `#t` if `x` is a string.
- `(symbol? x)`: Returns `#t` if `x` is an interned symbol.
- `(function? x)`: Returns `#t` if `x` is a native function or compiled Scheme closure.
- `(record? x)`: Returns `#t` if `x` is a record mapping.
- `(type-of x)`: Evaluates `x` and returns its type name as an interned symbol:
  ```lisp
  (type-of 10)       ; 'integer
  (type-of "hello")  ; 'string
  (type-of {a 1})    ; 'record
  ```

### List Manipulation Primitives

- `(cons head tail)`: Prepend `head` to the front of the list `tail`. If `tail` is not a list, constructs a new list containing `(head tail)`.
  ```lisp
  (cons 1 '(2 3)) ; (1 2 3)
  ```
- `(car list)`: Returns the first element of `list`. Triggers an error if empty.
  ```lisp
  (car '(10 20 30)) ; 10
  ```
- `(cdr list)`: Returns a new list containing all elements of `list` except the first. If the list has only one element, returns `nil`.
  ```lisp
  (cdr '(10 20 30)) ; (20 30)
  ```
- `(nth list index)`: Accesses the element at 0-indexed position `index` in `list`. Returns `nil` if out of bounds.
  ```lisp
  (nth '(a b c) 1) ; 'b
  ```
- `(count x)`: Returns the integer length of list `x`, string `x`, or `0` for `nil`.
  ```lisp
  (count "hello") ; 5
  (count '(1 2))  ; 2
  ```
- `(list &args)`: Constructs a new list containing the evaluated arguments.
  ```lisp
  (list 1 2 3) ; (1 2 3)
  ```
- `(empty? x)`: Evaluates to `#t` if `x` is `nil`, an empty list `()`, or an empty string `""`.
  ```lisp
  (empty? '()) ; #t
  ```

### Record Primitives

Records (`{key val}`) are manipulated using these functional primitives. They return a new, updated record, maintaining persistent immutable semantics.

- `(rget record symbol)`: Retrieves the value associated with `symbol` key from `record`. Returns `nil` if missing.
  ```lisp
  (rget {a 10} 'a) ; 10
  ```
- `(rset record symbol value)`: Returns a new copy of `record` with the key `symbol` bound to `value`.
  ```lisp
  (rset {a 1} 'b 2) ; {a 1 b 2}
  ```
- `(rdel record symbol)`: Returns a copy of `record` with the key `symbol` removed.
  ```lisp
  (rdel {a 1 b 2} 'a) ; {b 2}
  ```
- `(rkeys record)`: Returns a list containing all the symbol keys of the record.
  ```lisp
  (rkeys {a 1 b 2}) ; (a b)
  ```
- `(rvals record)`: Returns a list containing all the values inside the record.
  ```lisp
  (rvals {a 1 b 2}) ; (1 2)
  ```
- `(rcontains? record symbol)`: Evaluates to `#t` if `symbol` is a key present inside `record`.
  ```lisp
  (rcontains? {a 1} 'a) ; #t
  ```

### System & File I/O (Message Passing)

`sel` provides direct interfaces to OS primitives and the host filesystem via message-passing dispatch symbols.

- `(system message &args)`:
  - `(system 'args)`: Returns a list of strings representing the CLI arguments passed to the script.
  - `(system 'getenv key-str)`: Retrieves an OS environment variable by key string. Returns `nil` if missing.
  - `(system 'sleep secs-int)`: Pauses process execution for `secs-int` seconds.
  - `(system 'exit code-int)`: Immediately terminates the `sel` process returning `code-int` status.
- `(file-system message &args)`:
  - `(file-system 'exists? path-str)`: Returns `#t` if file exists at `path-str`.
  - `(file-system 'read path-str)`: Reads the entire contents of a file at `path-str` and returns it as a string.
  - `(file-system 'write path-str content-str)`: Writes `content-str` to a file at `path-str`. Returns `nil` if successful.

### Output & Logging

- `(display x)`: Prints the string representation of `x` to standard output without a newline.
- `(println x)`: Prints `x` followed by a newline.
- `(newline)`: Prints a single newline.
- `(error msg &args)`: Immediately halts evaluation and throws a runtime exception with a descriptive error message.

---

## 2. Scheme Standard Library (core.scm)

These routines and macros are defined in the standard library file `core.scm` and loaded during interpreter initialization.

### Logical Control & Assertions

- `(when test &body)`: *Macro*. If `test` is truthy, executes `body` expressions sequentially inside a `begin` block.
  ```lisp
  (when (= 1 1)
    (println "Math holds")
    (println "True!"))
  ```
- `(unless test &body)`: *Macro*. If `test` is falsy, executes `body` expressions sequentially.
  ```lisp
  (unless (= 1 2)
    (println "Inequal"))
  ```
- `(cond &xs)`: *Macro*. Multi-branch conditional selection. Takes pairs of test/consequent expressions. Runs the first matching condition. If the final test is `#t`, it acts as a default block.
  ```lisp
  (cond
    (= x 1) "One"
    (= x 2) "Two"
    #t      "Other")
  ```
- `(assert test &args)`: Evaluates `test`. If it evaluates to `#f` or `nil`, throws a runtime error. Optionally prints additional argument context.
  ```lisp
  (assert (= 2 2) "Math is broken!")
  ```

### Iteration Loops

These macros allow sequential looping constructs natively using TCO recursions.

- `(while test &body)`: *Macro*. Loops and executes `body` continuously as long as `test` evaluates to truthy.
  ```lisp
  (define i 0)
  (while (< i 5)
    (println i)
    (set! i (+ i 1)))
  ```
- `(until test &body)`: *Macro*. Loops and executes `body` continuously until `test` evaluates to truthy.
  ```lisp
  (define i 0)
  (until (= i 5)
    (println i)
    (set! i (+ i 1)))
  ```
- `(repeat f n)`: Executes a zero-argument function `f` exactly `n` times recursive-style.

### Functional List Utilities

- `(map f l)`: Applies a one-argument function `f` to each element in list `l`, returning a new list of results.
  ```lisp
  (map \(x) (* x x) '(1 2 3)) ; (1 4 9)
  ```
- `(filter f l)`: Returns a new list containing elements from list `l` for which `(f element)` evaluates to truthy.
  ```lisp
  (filter even? '(1 2 3 4)) ; (2 4)
  ```
- `(foldl f acc l)`: Left-associative list fold (reduce). Accumulates list values starting from the left.
  ```lisp
  (foldl + 0 '(1 2 3)) ; 6
  ```
- `(foldr f acc l)`: Right-associative list fold (reduce). Accumulates list values starting from the right.
- `(reverse l)`: Reverses the elements of list `l`.
- `(range &args)`: Convenient number sequence generator.
  - `(range end)`: Evaluates sequence from `0` to `end - 1` with step `1`.
  - `(range end begin)`: Evaluates from `begin` to `end - 1` with step `1`.
  - `(range end begin step)`: Evaluates from `begin` to `end - 1` in increments of `step`.
  ```lisp
  (range 5)       ; (0 1 2 3 4)
  (range 5 2)     ; (2 3 4)
  (range 10 2 2)  ; (2 4 6 8)
  ```
- `(even? x)`: Returns `#t` if `x` is divisible by `2`.

### List Manipulation

- `(last l)`: Returns the final element of list `l`.
- `(append l1 l2)`: Concatenates list `l1` and `l2` into a single list.
  ```lisp
  (append '(1 2) '(3 4)) ; (1 2 3 4)
  ```

### Promises & Laziness

Allows deferred lazy evaluation patterns.

- `(delay expr)`: *Macro*. Wraps `expr` inside a zero-argument lambda promise to avoid immediate evaluation.
- `(force promise)`: Forces evaluation of a lazy promise.
  ```lisp
  (define lazy-value (delay (+ 10 20))) ; Not evaluated yet
  (force lazy-value)                    ; Evaluates and returns 30
  ```

### Monadic Error Types

Allows functional error-handling patterns without stack-unwinding `try/catch` clauses. Monads return structures tagged with `'ok` or `'err`.

- `(ok val)`: Wraps a successful value into `(ok val)`.
- `(err msg)`: Wraps an error message into `(err msg)`.
- `(ok? x)`: Evaluates to `#t` if `x` is an ok container.
- `(err? x)`: Evaluates to `#t` if `x` is an error container.
- `(unwrap x)`: Extracts the value inside an ok container. Throws an exception if `x` is an error container.
- `(error-value x)`: Extracts the error message inside an err container.
- `(attempt expr)`: *Macro*. Wraps the evaluation of `expr` inside a try-catch, returning `(ok result)` if successful, or `(err exception)` if it failed.
  ```lisp
  (define res (attempt (/ 1 0))) ; (err "Called division by zero")
  ```
- `(try-bind val var body)`: *Macro*. Evaluates monadic `val`. If it is an error container, immediately returns that container; if it is successful, binds the unwrapped value to `var` and evaluates the `body` block. Perfect for chaining monadic calls.

### Record Extensions

- `(assoc record k v)`: Wrapper for native `rset`. Binds key `k` to value `v` inside `record`.
- `(dissoc record k)`: Wrapper for native `rdel`. Removes key `k` from `record`.

### Syntactic Helpers

- `(defun name args &body)`: *Macro*. Defines a standard function named `name`. Automatically wraps the `body` inside a `begin` block, allowing clean multi-line function declarations.
  ```lisp
  (defun welcome (name)
    (println "Initializing...")
    (println "Hello, " name))
  ```
- `(ffi-func symbol return-type argument-types)`: *Macro*. Conveniently maps a raw FFI symbol pointer to an anonymous Scheme lambda, allowing the library symbol to be called like any standard Scheme function.
  ```lisp
  (define puts-fn (ffi-func (ffi-dlsym libc "puts") 'i32 '(*u8)))
  (puts-fn "Called easily via Scheme wrapper!")
  ```
