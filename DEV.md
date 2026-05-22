# Developer & Contributing Guide

Welcome to the `sel` developer guide! This document outlines how to set up the development environment, understand the codebase structure, run tests, and contribute changes.

---

## 1. Setup & Requirements

To build and work on the compiler and interpreter, you will need:
- **Rust Toolchain**: `sel` uses Rust (edition `2024`). Ensure you have the latest stable Rust installed:
  ```bash
  rustup update stable
  ```
- **Node.js & npm** (optional): Needed only for building or previewing the documentation website.

---

## 2. Getting Started

### Building the Project
Compile the interpreter in debug mode:
```bash
cargo build
```

Compile a highly optimized release build:
```bash
cargo build --release
```

### Running the REPL Locally
You can start the interactive REPL shell immediately using Cargo:
```bash
cargo run
```

### Running Scheme Scripts
Run any `.scm` script file using the compiled binary:
```bash
cargo run -- examples/hello.scm
```

---

## 3. Codebase Architecture Overview

The source code is organized into modular components under `src/`:

- **`src/main.rs`**: System entry point, argument parsing via the CLI module, and the core rustyline-powered REPL loop.
- **`src/lexer.rs`**: Tokenizer that converts raw S-expression string buffers into streamable lexical tokens.
- **`src/parser.rs` & `src/ast.rs`**: S-expression AST generator. Handles structural parenthesis matching, quotes, and literal types.
- **`src/compiler.rs`**: Bytecode compiler. Lowers parsed AST forms into linear VM bytecode instruction chunks (`Chunk` / `OpCode`). Bytecode is serialized into a flat linear byte buffer (`Vec<u8>`) with explicit compile-time jump patching to resolve control flow offsets, reducing execution overhead.
- **`src/runtime.rs`**: Stack-based virtual machine evaluator. Contains the main instruction dispatch loop, environment binding maps, scope structures, tail call optimization (TCO), try/catch blocks, and coroutines. Local variables are compiled into stack index offsets and resolved via highly efficient `LoadLocal` and `StoreLocal` opcodes, bypassing dynamic environment map lookups.
- **`src/value.rs`**: Primitive and compound value types (e.g. Lists, Records, Closures, Macros, Coroutines) and their coercion utilities.
- **`src/diagnostics.rs`**: Lexical, compile-time syntax, and runtime exception systems with line-number context reporting.

---

## 4. Running the Test Suite

`sel` uses a dynamic integration test runner. Tests are written in Scheme and located in the `tests/` directory:

- Run all unit and Lisp integration tests:
  ```bash
  cargo test
  ```

*Note: The test suite dynamically scans `tests/` for all `.scm` files and executes them in fresh sandbox environments. It also validates negative cases in `tests/errors/` to ensure syntax and runtime errors are thrown correctly.*

---

## 5. Development Workflow & Guidelines

- **Code Formatting**: Ensure all code is cleanly formatted with standard Rust guidelines. Always run this before submitting a pull request:
  ```bash
  cargo fmt
  ```
- **Documentation**: If you change language features, primitives, or REPL commands, update the corresponding documentation files:
  - Markdown reference sheets: `docs/REFERENCE.md`, `docs/REPL.md`, and `docs/CORE.md`.
  - Astro docs: `docs/src/content/docs/reference.md`, `docs/src/content/docs/repl.md`, and `docs/src/content/docs/core.md`.

---

## 6. Developing the Docs Site Locally

The documentation website is built with Astro and Starlight. To edit the docs:

1. Navigate to the docs folder:
   ```bash
   cd docs
   ```
2. Install local Node dependencies:
   ```bash
   npm install
   ```
3. Run the development server:
   ```bash
   npm run dev
   ```
4. Build the static distribution to verify there are no markup or frontmatter errors:
   ```bash
   npm run build
   ```
