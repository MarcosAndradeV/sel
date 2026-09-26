# sel Lisp

`sel` is a fast, embeddable, and extensible Scheme-like Lisp dialect implemented in Rust. It combines traditional functional paradigms with a modern dynamically-typed runtime, allowing for powerful metaprogramming, tail call optimization, and native integration through a zero-boilerplate dynamic Foreign Function Interface (FFI).

---

## Key Features

- **Dual-Syntax Ecosystem**:
  - **Classic Scheme (`.scm`)**: Homoiconic S-expressions, `define`, macros, and full Lisp flexibility.
  - **Modern Functional (`.sel`)**: Indentation-friendly ML/Elixir-inspired syntax featuring `:=` definitions, multi-clause pattern-matching functions with guards (`when`), thread-first (`|>`) and thread-last (`|>>`) pipelines, record dot-access, and native `.sel` stdlib prelude.
- **First-Class Functions & TCO**: Full first-class function support with tail call optimization for infinite recursion without stack overflows.
- **True Metaprogramming**: A macro system (`defmacro`) with quasiquotation (`` ` ``), unquote (`~`), and unquote-splicing (`~@`).
- **Modern Primitives**: Native support for **curly-brace record structures** (`{key: value}` or `{key value}`), multi-base numbers (`0xFF`, `0b1010`, `0o755`), thread pipelines, and native coroutines.
- **Dynamic FFI**: Zero-boilerplate runtime integration with standard C shared libraries using `libffi`.

---

## Quick Start

### Installation
To build and install the interpreter CLI, ensure you have Rust/Cargo installed:

```bash
cargo install --path . --features alt-syntax
```

### Running Scripts & REPL
Run a script directly (`.scm` or `.sel`):
```bash
sel examples/hello.scm
sel examples/modern_functional.sel
```

Or start the interactive REPL:
```bash
sel
# or with alternative syntax mode:
sel -a
```

---

## Language Showcase

### 1. Modern Functional Syntax (`.sel`)
```sel
// Multi-clause pattern matching with guards
fib 0 := 0
fib 1 := 1
fib n := fib(n - 1) + fib(n - 2)

// Pipelines and collections
even_squares_sum := [1, 2, 3, 4, 5, 6]
    |> filter_by(\x -> is_even(x))
    |> map_by(\x -> x * x)
    |> sum // 56

// Records with dot-access
user := { id: 101, name: "Alice", active: true }
println(user.name) // "Alice"
```

### 2. Classic Scheme Lisp Syntax (`.scm`)
```lisp
;; Infinite recursion safe (TCO)
(define (countdown n)
  (if (= n 0)
      "Done!"
      (countdown (- n 1))))

;; Dynamic FFI calls out-of-the-box
(define libc (ffi-dlopen "libc.so.6"))
(define puts (ffi-dlsym libc "puts"))
(ffi-call puts 'i32 '(*u8) "Hello from C puts!")

;; Sleek record data structures
(define person {name "Marcos" age 30})
(print (rget person 'name)) ; "Marcos"
```

---

## Documentation

Comprehensive documentation has been structured inside the `docs/` directory:

- **[Language Reference](docs/src/content/docs/reference.md)**: Full syntax, types, base conversions, special forms, FFI rules, and execution models.
- **[Modern Functional Syntax (.sel)](docs/src/content/docs/alt_syntax.md)**: Full reference for the `.sel` syntax, multi-clause pattern matching, thread-first/thread-last pipelines, record access, and `.sel` prelude.
- **[Interactive REPL](docs/src/content/docs/repl.md)**: Guide to using the interactive REPL shell, special environment commands, variable inspection, loading Scheme scripts, and CLI features.
- **[Standard & Core Library](docs/src/content/docs/core.md)**: References for both the native internal built-ins (implemented in Rust) and the Scheme-defined standard library modules (TCO loops, functional maps, monadic errors, etc.).

### Web-Based Documentation Viewer (Starlight)
`sel` docs include a fully configured, ultra-fast **Astro Starlight** documentation site. 

To view the documentation in a beautiful, responsive, and search-indexed web interface locally:

1. Navigate to the `docs` folder:
   ```bash
   cd docs
   ```
2. Install node dependencies:
   ```bash
   npm install
   ```
3. Run the development server:
   ```bash
   npm run dev
   ```
4. Open the displayed URL (usually `http://localhost:4321`) in your browser.
