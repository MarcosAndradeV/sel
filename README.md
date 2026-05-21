# sel Lisp

`sel` is a fast, embeddable, and extensible Scheme-like Lisp dialect implemented in Rust. It combines traditional functional paradigms with a modern dynamically-typed runtime, allowing for powerful metaprogramming, tail call optimization, and native integration through a zero-boilerplate dynamic Foreign Function Interface (FFI).

---

## Key Features

- **Classic Lisp Syntax**: S-expressions everywhere! Supported special forms like `define`, `lambda`, `if`, `let`, `set!`, and `begin`.
- **First-Class Functions & TCO**: Full first-class function support with tail call optimization for infinite recursion without stack overflows.
- **True Metaprogramming**: A macro system (`defmacro`) with quasiquotation (`` ` ``), unquote (`~`), and unquote-splicing (`~@`).
- **Modern Primitives**: Native support for **curly-brace record structures** (`{key value}`), multi-base numbers (`0xFF`, `0b1010`, `0o755`), thread-first pipelines (`->`), and native coroutines.
- **Dynamic FFI**: Zero-boilerplate runtime integration with standard C shared libraries using `libffi`.

---

## Quick Start

### Installation
To build and install the interpreter CLI, ensure you have Rust/Cargo installed:

```bash
cargo install --path .
```

### Running Scripts & REPL
Run a script directly:
```bash
sel examples/hello.scm
```

Or start the interactive REPL:
```bash
sel
```

---

## Language Showcase

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

- **[Language Reference](docs/REFERENCE.md)**: Full syntax, types, base conversions, special forms, FFI rules, and execution models.
- **[Interactive REPL](docs/REPL.md)**: Guide to using the interactive REPL shell, special environment commands, variable inspection, loading Scheme scripts, and CLI features.
- **[Standard & Core Library](docs/CORE.md)**: References for both the native internal built-ins (implemented in Rust) and the Scheme-defined standard library modules (TCO loops, functional maps, monadic errors, etc.).

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
