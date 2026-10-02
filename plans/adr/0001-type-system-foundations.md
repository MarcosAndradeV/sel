# ADR 0001: Type System Foundations, Enforcement Posture, and Declaration Syntax

## Status
Accepted (Agreed during Grill Session #1, 2026-10-01)

## Context
SEL is introducing an expressive type system supporting parametric polymorphism (`forany`), ad-hoc polymorphism (`implements`, `derive`), and nominal types (`newtype`).

Prior exploration left open several foundational questions:
1. What is the compiler posture on type mismatches (strict static compilation gate vs. gradual/advisory warnings)?
2. Where and how should nominal types and traits be declared (a special `comptime { ... }` block vs. top-level declarative statements)?
3. How should nominal types be instantiated (explicit `cast(Type, expr)` vs. standard constructor function calls `Type(expr)`)?

## Decisions

### 1. Strict Static Gate (Option 1A)
- The type checker runs as a mandatory compilation phase immediately after AST resolution/macro expansion and pattern matching analysis, before bytecode emission.
- Any type mismatch, unfulfilled trait constraint, or invalid signature emits a hard `SelError::TypeError` (or diagnostic compilation error) and halts compilation. No bytecode chunk or VM execution is performed on failed checks.

### 2. Top-Level Declarations (Elimination of `comptime` Block)
- The `comptime { ... }` block is eliminated as redundant.
- Type definitions and trait mappings are top-level declarative statements:
  - `newtype Foo := { c: int }` (or single-field unboxed `newtype ID := int`)
  - `derive(Foo, :comparable)`
  - `implements(Foo, :comparable, \a, b -> a.c == b.c)`
- These top-level statements register nominal types and trait instances directly into the module's `TypeEnv`.

### 3. Constructor Calls for Nominal Types
- `cast(Type, expr)` is replaced with standard functional constructor calls: `Foo({ c: 10 })`.
- `newtype Foo := ...` introduces both:
  1. A nominal type identity `Foo` in the type universe.
  2. A constructor function `Foo : UnderlyingType -> Foo` callable in SEL code.
- Field access on nominal types (`foo.c`) transparently accesses fields of the underlying representation.
- At runtime, with static safety guaranteed by Phase 1A, representation can be zero-overhead or minimally tagged for nominal equality/trait dispatch.

## Consequences & Tradeoffs
- **Pros**:
  - Clean, unpolluted syntax without awkward `comptime` wrappers.
  - Familiar constructor ergonomics (`Foo(...)`).
  - Total static confidence before runtime.
- **Cons / Risks**:
  - Functions in typed modules must satisfy static checks, requiring clear type inference or signatures for imported/prelude functions.
