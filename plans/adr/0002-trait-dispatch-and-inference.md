# ADR 0002: Runtime Nominal Tagging and Public Signature Enforcement

## Status
Accepted (Agreed during Grill Session #2, 2026-10-01)

## Context
Following ADR 0001 (Strict Static Gate Option 1A, top-level type definitions, and constructor functions), two key operational questions needed resolution:
1. How should trait implementations (specifically ad-hoc equality and operators) be dispatched at runtime for user-defined types?
2. What is the scope and requirement for type annotations across a codebase (e.g. inference depth for untyped functions vs. exported boundaries)?

## Decisions

### 1. Runtime Nominal Tagging for Custom Trait Dispatch
- Nominal values created via constructor functions (`Foo(val)`) are wrapped at runtime as `Value::Nominal(Rc<NominalValue>)` containing:
  - `type_id`: Symbol ID representing the nominal type identity (`Foo`).
  - `inner`: The underlying wrapped value (e.g. Record `{ c: 10 }`, integer, etc.).
- The VM's equality operator (`==`) and future trait operations check for registered trait handlers (e.g. `:comparable`) matching `type_id`.
  - If a custom handler was registered via `implements(Foo, :comparable, fn)`, it is executed.
  - If `derive(Foo, :comparable)` was specified, it performs recursive member-wise equality.
- Dot-access (`foo.c`) on `Value::Nominal` transparently unwraps to the inner value with zero friction.

### 2. Mandatory Signatures for `pub` Exports (Option B)
- All public, exported functions (`pub name ...`) MUST declare an explicit top-level type signature (`pub name :: Sig` or `name :: Sig`).
  - Attempting to export a function without a type signature results in a compile-time static type error (`SelError::TypeError`).
- Private and local functions, as well as lambda expressions, undergo bidirectional type inference and are validated against call sites.
- This creates clean, documented, self-describing module boundaries while preserving rapid iteration for internal helper functions.

## Consequences & Tradeoffs
- **Pros**:
  - High interface clarity: Public APIs are guaranteed to have explicit, type-checked documentation.
  - Dynamic flexibility in VM: Nominal values carry their identity, allowing accurate runtime error reporting and custom trait dispatch.
  - Low cognitive burden for internal code: Private helpers can omit verbose signatures.
- **Cons / Risks**:
  - Existing scripts in `examples/` and `tests/` that use `pub` without `::` will need explicit signatures if analyzed under the strict static gate, or module loading must phase this enforcement.
