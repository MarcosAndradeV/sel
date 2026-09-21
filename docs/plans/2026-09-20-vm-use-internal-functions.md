# VM `run_internal` Builtin Delegation & Inlining Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor `VM::run_internal` in `src/runtime.rs` to delegate all builtin opcodes to the corresponding functions in `src/internal.rs`, and annotate these functions with `#[inline]` in `src/internal.rs` so features only need to be implemented once with optimal runtime performance.

**Architecture:** Instead of maintaining duplicate logic in `run_internal` for opcodes (`Mod`, `Cons`, `Car`, `Cdr`, `Nth`, `Count`, `Empty`, `IsNil`, `IsList`, `IsNumber`, `IsString`, `IsSymbol`, `IsFunction`, `TypeOf`, `MakeList`), `run_internal` drains or pops its operands from `self.stack` and invokes `internal::<func>(loc, args)?`. Functions in `src/internal.rs` are annotated with `#[inline]` to allow the compiler to inline them into `run_internal`'s opcode dispatch loop.

**Tech Stack:** Rust (2024 edition / 1.85+), Sel Bytecode VM.

---

### Task 1: Add `#[inline]` to builtin functions in `src/internal.rs`

**Files:**
- Modify: `src/internal.rs`

**Step 1: Minimal changes in `src/internal.rs`**
Add `#[inline]` to:
- `div`, `modulo`, `compare_nums`, `is_equal`, `num_noteq`, `num_eq`, `num_lt`, `num_gt`, `num_lte`, `num_gte`
- `error`, `value_type_name`, `not`, `cons`, `car`, `cdr`, `nth`, `drop`, `count`, `list`, `empty`
- `rget`, `rset`, `rdel`, `rkeys`, `rvals`, `rcontains`
- `is_nil`, `is_list`, `is_number`, `is_string`, `string_contains`, `is_symbol`, `gensym`, `is_record`, `is_function`, `is_char`
- `char_to_integer`, `integer_to_char`, `type_of`
Add arity check `if args.len() != 2 { return Err(SelError::Runtime(loc, "Expected 2 arguments for mod".into())); }` to `modulo`.

**Step 2: Run cargo check**
Run: `cargo check`
Expected: PASS

---

### Task 2: Refactor `run_internal` in `src/runtime.rs` to delegate to `internal`

**Files:**
- Modify: `src/runtime.rs:868-1115`

**Step 1: Replace duplicated opcode arms in `run_internal`**
- Opcode 25 (`MakeList`): `internal::list`
- Opcode 31 (`Mod`): `internal::modulo`
- Opcode 39 (`Cons`): `internal::cons`
- Opcode 40 (`Car`): `internal::car`
- Opcode 41 (`Cdr`): `internal::cdr`
- Opcode 42 (`Nth`): `internal::nth`
- Opcode 43 (`Count`): `internal::count`
- Opcode 44 (`Empty`): `internal::empty`
- Opcode 45 (`IsNil`): `internal::is_nil`
- Opcode 46 (`IsList`): `internal::is_list`
- Opcode 47 (`IsNumber`): `internal::is_number`
- Opcode 48 (`IsString`): `internal::is_string`
- Opcode 49 (`IsSymbol`): `internal::is_symbol`
- Opcode 50 (`IsFunction`): `internal::is_function`
- Opcode 51 (`TypeOf`): `internal::type_of`

**Step 2: Run cargo test**
Run: `cargo test`
Expected: PASS

---

### Task 3: Add test for `car` on string and verify end-to-end

**Files:**
- Create: `tests/test_car_string.scm`

**Step 1: Write test in `tests/test_car_string.scm`**
```scheme
(assert (eq? (car "hello") #\h))
(assert (eq? (car "world") #\w))
```

**Step 2: Run test suite**
Run: `cargo test`
Expected: PASS (all tests pass including tests folder)

---

## Verification Plan

### Automated Tests
- `cargo test`
- `cargo check --release`

### Manual Verification
- `cargo run -- -e '(car "hello")'` -> evaluates to `#\h`
- `cargo run -- -e '(mod 10 3)'` -> evaluates to `1`
