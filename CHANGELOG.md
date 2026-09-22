# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Hierarchical Module System & Search Paths:
  - Added support for hierarchical module specs in `(import ...)`: `(import std/math)`, `(import "pkg/mod" :as p)`.
  - Configured search path precedence: caller file's directory, current working directory, and colon-separated directories in `SEL_PATH`.
  - Added package directory import support via `<path>/mod.scm` convention with automatic module prefix inference from the enclosing directory.
- Compact String Sequence Architecture:
  - Replaced heap-allocated character vector lists for strings with an optimized `StringSlice` struct and `Value::String` variant backed by `Rc<str>` with byte offset windows.
  - Transparent list-of-char sequence polymorphism: `car`, `cdr`, `cons`, `nth`, `drop`, `count`, and `empty?` operate on strings without allocating character vectors.
  - Both `(list? s)` and `(string? s)` return `#t` for strings.
  - Full pattern matching interoperability: `(cons h t)`, fixed lists, and multi-clause `defn` pattern clauses seamlessly destructure strings.
  - Multi-byte UTF-8 boundary handling for all slice operations.
- Multiline interactive REPL support with automatic delimiter tracking (`()`, `[]`, `{}`), string literal escape awareness, and continuation prompt (`  ..> `).
- Standard Math library built-ins:
  - Numerical utilities: `abs`, `min`, `max`, `sqrt`, `pow`, `floor`, `ceil`, `round`.
  - Trigonometric operations: `sin`, `cos`, `tan`.
  - Bitwise integer operations: `bit-and`, `bit-or`, `bit-xor`, `bit-not`, `bit-shl`, `bit-shr`.
  - Standard constants in core library: `pi`, `tau`, `e`.
- Standard String utility library:
  - `string-split`, `string-join`, `string-trim`, `string-replace`.
  - `string-upcase`, `string-downcase`.
  - `to-string` (universal stringification) and `format` (sequential `{}` placeholder interpolation).
- System, OS, and Time primitives:
  - `get-env` and `set-env!` for process environment variable inspection and mutation.
  - `time-now-ms` (UNIX epoch milliseconds) and `sleep-ms` (thread sleeping).
  - Extended filesystem primitives: `fs-list` and `fs-delete` with Scheme helper wrappers.
- Integration test suites under `tests/`: `test_math.scm`, `test_string_utils.scm`, and `test_system.scm`.
- Comprehensive pattern matching system (`match`, `match-lambda`) supporting:
  - Literals (numbers, strings, booleans, nil, symbols)
  - Wildcards (`_`) and variable bindings
  - List destructuring with rest patterns (`(h . t)` or `(h & t)`)
  - Vector destructuring (`[x y z]`)
  - Record destructuring with field binding and punning (`%{name, role: r}`)
  - Guard expressions via `:when` or `:where`
  - Deep nested pattern composition and exhaustiveness verification
- Elixir, Erlang, and OCaml-inspired functional UX syntax and control-flow macros:
  - Pipeline operators: `|>` (thread-last for collection pipelines) and `->` (thread-first for records and dictionaries).
  - Multi-clause function definitions (`defn`) with pattern matching, guard expressions (`:when`/`:where`), and `:do` syntax.
  - Railway-oriented `with` macro for chaining monadic/fallible operations with `:do` bodies and `:else` failure dispatch.
  - Comprehensive list comprehensions (`for`) supporting multiple generators, intermediate `:let` bindings, `:when`/`:where` filter guards, and `:do` collection expressions.
  - Core helper `string-contains?` for substring inspection.
- Configured a Rust library target in `Cargo.toml` (`src/lib.rs`) allowing `sel` to be embedded inside host applications (e.g. game or graphic engines). Exposes `eval`, `Env`, `Value`, and core bindings.
- Added `(load "script.scm")` built-in form to dynamically evaluate S-expression files inside the caller's active lexical environment.
- Implemented robust integration tests (`tests/test_load.scm` and `tests/helper_load.scm`) for file-system loading, binding visibility, TCO, and error handling.
- Introduced a dedicated `SelError::SandboxViolation` variant to the diagnostic error system.
- Added directory traversal security checks mapping file load and import calls within a configurable sandbox root. Exposes `eval_sandboxed` and `load_file_sandboxed`.
- Added a `ffi` Cargo feature flag (enabled by default) to compile out dynamic loading dependencies (`libloading`, `libffi`) and their associated built-ins for sandboxed host environments.
- Added programmatic accessors to `SelError` for embedders: `.kind()` returning a new `SelErrorKind` categorization, `.loc()` extracting the error span, and `.message()` returning location-free causal messages.

### Changed
- Redesigned macro compiler logic to use a postponed AST resolution pass (`resolve_ast`), allowing special forms (like `let`, `when`, `unless`, `if`) to be constructed seamlessly inside quasiquoted macro bodies (e.g. using `~` and `~@`) without triggering parse-time syntax verification.
- Explicitly wrapped `while` and `until` bodies in an `Ast::List` rather than compiling raw statements directly, preventing compiler panics when the first statement in the body matches a special form keyword.
- Fixed `Ast::Try` compiler locals registration and scope depth tracking in catch blocks, ensuring accurate local variable resolution in nested closures.
