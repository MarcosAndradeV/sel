# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Configured a Rust library target in `Cargo.toml` (`src/lib.rs`) allowing `sel` to be embedded inside host applications (e.g. game or graphic engines). Exposes `eval`, `Env`, `Value`, and core bindings.
- Added `(load "script.scm")` built-in form to dynamically evaluate S-expression files inside the caller's active lexical environment.
- Implemented robust integration tests (`tests/test_load.scm` and `tests/helper_load.scm`) for file-system loading, binding visibility, TCO, and error handling.

### Changed
- Redesigned macro compiler logic to use a postponed AST resolution pass (`resolve_ast`), allowing special forms (like `let`, `when`, `unless`, `if`) to be constructed seamlessly inside quasiquoted macro bodies (e.g. using `~` and `~@`) without triggering parse-time syntax verification.
- Explicitly wrapped `while` and `until` bodies in an `Ast::List` rather than compiling raw statements directly, preventing compiler panics when the first statement in the body matches a special form keyword.
