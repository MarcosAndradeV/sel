# SEL

`sel` is a fast, embeddable, and extensible modern functional programming language implemented in Rust. It combines clean ML- and Elixir-inspired syntax with a high-performance bytecode VM, tail call optimization, cooperative coroutines, and native integration through a zero-boilerplate dynamic Foreign Function Interface (FFI).

---

## Key Features

- **Modern Functional Syntax (`.sel`)**: Indentation-friendly syntax featuring `:=` definitions, multi-clause pattern-matching functions with guards (`when`), thread-first (`|>`) and thread-last (`|>>`) pipelines, record dot-access, and a comprehensive stdlib prelude.
- **First-Class Functions & TCO**: Full first-class function support with tail call optimization for recursion without stack overflows.
- **Modern Primitives**: Native support for **curly-brace record structures** (`{ key: value }`), multi-base numbers (`0xFF`, `0b1010`, `0o755`), thread pipelines, and cooperative coroutines (`co_create`, `yield`, `co_resume`).
- **Dynamic FFI**: Zero-boilerplate runtime integration with standard C shared libraries using `libffi`.
- **Lightweight & Embeddable**: Simple Rust API to embed, evaluate expressions, and load modules in sandboxed environments.

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
sel examples/modern_functional.sel
```

Or start the interactive REPL:
```bash
sel
```

---

## Language Showcase

### Modern Functional Syntax (`.sel`)
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

// Cooperative coroutines
generator := co_create(\start -> do
    next := yield(start + 10)
    next * 2
end)
```

---

## Documentation

Comprehensive documentation has been structured inside the `docs/` directory:

- **[Language Reference](docs/src/content/docs/reference.md)**: Full syntax, types, base conversions, definitions, FFI rules, and execution models.
- **[Interactive REPL](docs/src/content/docs/repl.md)**: Guide to using the interactive REPL shell, environment inspection, module loading, and CLI features.
- **[Standard & Core Library](docs/src/content/docs/core.md)**: References for both the native internal built-ins and the standard library prelude (`core.sel`).

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
