# sel Language

`sel` is a fast, embeddable, and extensible Scheme-like Lisp dialect implemented in Rust. It combines traditional functional paradigms with a modern dynamically-typed runtime, allowing for powerful metaprogramming, tail call optimization, and native integration through a powerful dynamic C Foreign Function Interface (FFI).

## Features

- **Classic Lisp Syntax**: S-expressions everywhere! Core special forms like `define`, `lambda`, `if`, `let`, `set!`, and `begin`.
- **First-Class Functions**: First-class support for closures and functional programming with full Tail Call Optimization (TCO).
- **Macros and Quasiquotation**: True Lisp macro system (`defmacro`) with quasiquotation (`` ` ``), unquote (`~`), and unquote-splicing (`~@`) support for robust metaprogramming.
- **Dynamic FFI**: Zero-boilerplate runtime integration with C libraries using `libffi` and `libloading`.
- **Numerical Bases**: Natively write Hexadecimal (`0xFF`), Binary (`0b1010`), Octal (`0o755`), and Decimal numbers.

## Getting Started

To build and install `sel`, you will need Rust installed on your system.

```bash
cargo install --path .
```

Now you can execute a `sel` script by passing it as an argument:
```bash
sel tests/test_ffi.scm
```

Or run the interactive REPL:
```bash
sel
```

## Language Showcase

### Functions and Tail Call Optimization
`sel` properly handles recursive functional loops without blowing up the stack:

```lisp
(define (countdown n)
  (if (= n 0)
      "Done!"
      (countdown (- n 1))))

(print (countdown 1000000))
```

### Macros and Quasiquotation
Create your own language constructs using `defmacro` and quasiquotations:

```lisp
(defmacro (unless condition body)
  `(if ~condition nil ~body))

(unless (= 1 2) 
  (print "Math is still working!"))
```

### Different Number Bases
You can write numeric literals using different bases, which will automatically be parsed as internal integers:

```lisp
(define flags 0b101101)
(define mask 0xFF)
(define permissions 0o755)
```

### System and File I/O (Message Passing)
`sel` provides an idiomatic, closure-based message passing API for OS and file system interactions using the `system` and `file-system` native functions.

```lisp
;; OS interactions
(system 'getenv "USER") ;; Returns the environment variable or nil
(system 'args)          ;; Returns a list of command line arguments
(system 'sleep 1)       ;; Sleeps for 1 second
(system 'exit 0)        ;; Exits the process

;; File system interactions
(file-system 'write "test.txt" "Hello World")
(file-system 'exists? "test.txt")
(file-system 'read "test.txt")
```

### Dynamic Foreign Function Interface (FFI)
Call native shared objects (`.so` / `.dll` / `.dylib`) directly from your `sel` scripts without writing any Rust bindings. The FFI layer maps strings to C-strings and coercions automatically.

```lisp
;; Load libc and find symbols
(define libc (ffi-dlopen "libc.so.6"))
(define strlen (ffi-dlsym libc "strlen"))
(define puts (ffi-dlsym libc "puts"))

;; Invoke the functions!
(print (ffi-call strlen 'u64 '(*u8) "Hello, FFI!")) ; Evaluates to 11

(ffi-call puts 'i32 '(*u8) "This string is printed natively by C!")

;; Work with floats and other libraries
(define libm (ffi-dlopen "libm.so.6"))
(define my_pow (ffi-dlsym libm "pow"))
(print (ffi-call my_pow 'f64 '(f64 f64) 2.0 3.0)) ; Evaluates to 8.0
```

### FFI Supported Types
The `ffi-call` signature is `(ffi-call <pointer> <ret-type> (<arg-types>) <args...>)`. It supports the following C type mappings:
- **`'void`**: (Return type only) Returns `nil` in `sel`.
- **`'bool`**: Maps directly to `sel` `Value::Boolean`.
- **`'i32`, `'i64`, `'u32`, `'u64`**: Evaluates `sel` numbers and casts them to integer widths.
- **`'f32`, `'f64`**: Maps `sel` floats.
- **`'*u8`**: A pointer type. If passed a string, it automatically creates a null-terminated C-string array and manages its lifetime for the duration of the call.

## Standard Library (core.scm)

Many familiar higher-order functions like `map`, `reduce`, `filter`, and basic primitives are loaded globally. Check `core.scm` for definitions.
