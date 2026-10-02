# SEL Type System Domain Glossary

This glossary establishes the canonical ubiquitous language and domain model for SEL's type system, formalizing terminology across parsing, type checking, compile-time evaluation, and runtime semantics.

---

## 1. Type Classification & Semantics

### Structural Type
A type equivalence model where two types are considered equal if their structure, shape, and component types match, regardless of declaration name or origin. In SEL, raw records (e.g. `{ c: int, name: string }`) and tuples are purely structural.

### Nominal Type (`newtype`)
A type equivalence model where a type possesses an explicit, unique identity established at declaration time. Even if `Foo` has the exact representation `{ c: int }`, a value of type `{ c: int }` is not interchangeable with `Foo` without an explicit conversion (`cast` or constructor invocation).

### Reified Nominal Value (`Value::Nominal`)
A runtime representation holding a type identifier tag (`type_id`) alongside the inner wrapped value. Enables runtime type introspection, dynamic pattern matching on nominal types, and nominal trait method dispatch.

### Type Erasure
The process of stripping type annotations, signatures, and assertions during or after compilation, leaving pure untyped bytecode. In zero-cost erasure, types have zero runtime memory or execution overhead.

---

## 2. Polymorphism & Constraints

### Parametric Polymorphism (`forany(A, ...)`)
Universal quantification allowing functions or data structures to operate identically across any concrete type parameter without inspecting its concrete representation.

### Ad-Hoc Polymorphism (Traits)
Polymorphism allowing different concrete types to provide distinct implementations of a common contract or protocol (e.g. `:comparable`, `:orderable`, `:hashable`, `:showable`).

### Trait Bound (`where implements(T, :trait)`)
A static constraint on a universally quantified type variable `T`, asserting that any concrete type instantiated for `T` must provide a registered implementation of `:trait`.

### Trait Dictionary / Method Dispatch
The mechanism through which trait method implementations are resolved:
- **Static / Monomorphized**: Resolved and specialized at compile-time (zero runtime lookup).
- **Dictionary Passing**: Trait method tables are implicitly passed as hidden arguments.
- **Dynamic VM Lookup**: Trait methods are looked up dynamically via the runtime `type_id` and trait atom registry.

---

## 3. Declarative Model & Static Analysis

### Top-Level Type Declarations
Type and trait declarations (`newtype`, `derive`, `implements`) reside directly at the module top level alongside functions and variables. They populate the module's `TypeEnv` during the parsing and static analysis phases.

### Nominal Constructor Function (`Foo(expr)`)
Declaring `newtype Foo := T` automatically generates a typed constructor function `Foo : T -> Foo` in scope, wrapping underlying values into the nominal type.

### Bidirectional Type Checking
A type-checking strategy split into two reciprocal phases:
1. **Inference / Synthesis** (`Γ ⊢ e ⇒ τ`): Computing a type from an expression.
2. **Checking** (`Γ ⊢ e ⇐ τ`): Verifying that an expression conforms to an expected type context, propagating type information inwards.

### Type Assertion (`expr :: Type`)
A compile-time static check verifying that the inferred or synthesized type of `expr` conforms to `Type`.

