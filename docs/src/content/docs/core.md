---
title: Standard & Core Library
description: Comprehensive reference for all native built-in functions (implemented in Rust) and the standard library prelude routines loaded inside the core runtime.
---

This document describes all functions and operators available globally in the `SEL` runtime environment. They are divided into **Native Built-in Functions** (implemented directly in the Rust interpreter core) and the **Standard Library Prelude** (defined in `core.sel`).

---

## 1. Native Built-in Functions (Rust-Implemented)

These core primitives are registered directly inside the environment by the interpreter runtime.

### Arithmetic & Numeric Operators

| Function | Arity | Description | Example |
| :--- | :--- | :--- | :--- |
| `+` | Variadic | Sums numeric arguments. | `1 + 2 + 3` -> `6` |
| `-` | > 1 | Subtraction or unary negation. | `10 - 3` -> `7`, `-5` -> `-5` |
| `*` | Variadic | Multiplies numeric arguments. | `2 * 3 * 4` -> `24` |
| `/` | 2 | Performs division (supports integers and floats). | `10 / 2` -> `5`, `5.0 / 2.0` -> `2.5` |
| `%` / `mod` | 2 | Remainder of dividing first argument by second. | `10 % 3` -> `1`, `mod(10, 3)` -> `1` |

### Equalities & Comparators

| Operator / Function | Arity | Description | Example |
| :--- | :--- | :--- | :--- |
| `==` | 2 | Checks structural equality (atoms, numbers, strings, lists, records). | `:a == :a` -> `true`, `[1, 2] == [1, 2]` -> `true` |
| `!=` | 2 | Structural inequality check. | `5 != 10` -> `true` |
| `<` | 2 | Strictly less than. | `2 < 3` -> `true` |
| `>` | 2 | Strictly greater than. | `5 > 3` -> `true` |
| `<=` | 2 | Less than or equal to. | `3 <= 3` -> `true` |
| `>=` | 2 | Greater than or equal to. | `5 >= 2` -> `true` |
| `!` / `not` | 1 | Logically negates the boolean value. | `!false` -> `true`, `not(nil)` -> `true` |

### Type Query & Reflection

Predicates that return `true` or `false`:

- `is_nil(x)`: Returns `true` if `x` is `nil`.
- `is_list(x)`: Returns `true` if `x` is a sequence list.
- `is_number(x)`: Returns `true` if `x` is an integer or float.
- `is_string(x)`: Returns `true` if `x` is a string.
- `string_contains(str, substr)`: Returns `true` if `str` contains `substr`.
- `is_symbol(x)`: Returns `true` if `x` is a symbol or atom.
- `is_function(x)`: Returns `true` if `x` is a function or closure.
- `is_record(x)`: Returns `true` if `x` is a record mapping.
- `is_char(x)`: Returns `true` if `x` is a character value.
- `type_of(x)`: Evaluates `x` and returns its type name as an atom:
  ```sel
  type_of(10)      // :int
  type_of("hello") // :string
  type_of({a: 1})  // :record
  type_of(#\a)     // :char
  ```
- `gensym([prefix])`: Generates a globally unique identifier (e.g. `g0`, `temp1`).

### Character Conversions

- `char_to_integer(ch)`: Returns the Unicode scalar integer for character `ch`.
  ```sel
  char_to_integer(#\a) // 97
  ```
- `integer_to_char(int)`: Returns the character for Unicode scalar integer `int`.
  ```sel
  integer_to_char(97) // #\a
  ```

### List Manipulation Primitives

- `cons(head, tail)`: Prepend `head` to front of `tail`.
- `car(list)`: Returns the first element of `list`.
- `cdr(list)`: Returns the list without its first element.
- `drop(n, list)`: Drops first `n` elements in $O(1)$ time without copying.
- `take(n, list)`: Takes first `n` elements.
- `nth(list, index)`: Retrieves element at 0-indexed position.
- `count(x)`: Returns length of a list, string, or `0` for `nil`.
- `list(...args)`: Constructs a list from arguments.
- `is_empty(x)`: Returns `true` if `x` is `nil`, `[]`, or `""`.

### Record Primitives

Records (`{ key: value }`) are immutable maps:

- `rget(record, key)`: Retrieves value by atom or string key.
- `rset(record, key, value)`: Returns updated record with key set.
- `rdel(record, key)`: Returns record with key removed.
- `rkeys(record)`: Returns list of keys.
- `rvals(record)`: Returns list of values.
- `rcontains(record, key)`: Returns `true` if key is present in record.

### String Manipulation Primitives

- `string_split(str, delimiter)`: Splits string by delimiter into a list of strings.
- `string_join(list, separator)`: Joins list of strings with separator.
- `string_trim(str)`: Trims leading and trailing whitespace.
- `string_replace(str, pattern, replacement)`: Replaces occurrences of pattern.
- `string_upcase(str)`: Converts string to uppercase.
- `string_downcase(str)`: Converts string to lowercase.
- `to_string(x)`: Converts value `x` to its string representation.
- `to_int(x)`: Converts integer, float, or numeric string to integer.
- `to_float(x)`: Converts integer, float, or numeric string to float.
- `format(fmt, ...args)`: Formats string using `{}` placeholders:
  ```sel
  format("Hello, {}! You have {} points.", "Alice", 100)
  ```

### Math Functions

- `abs(x)`: Absolute value.
- `min(a, b)` / `max(a, b)`: Minimum / maximum.
- `sqrt(x)`: Square root.
- `pow(base, exp)`: Power exponentiation.
- `floor(x)`, `ceil(x)`, `round(x)`: Rounding operations.
- `sin(x)`, `cos(x)`, `tan(x)`: Trigonometric functions.
- `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `bit_shl`, `bit_shr`: Bitwise operations.

### System & Environment

- `system(:args)`: CLI arguments list.
- `system(:exit, code)`: Terminate process with exit code.
- `get_env(key)`: Read environment variable.
- `set_env(key, val)`: Set environment variable.
- `time_now_ms()`: Current Unix timestamp in milliseconds.
- `sleep_ms(ms)`: Pause execution for specified milliseconds.
- `file_system(:exists, path)`: Checks if file exists.
- `file_system(:read, path)`: Reads file contents as string.
- `file_system(:write, path, content)`: Writes content to file.
- `file_system(:list, path)`: Lists directory contents.
- `file_system(:delete, path)`: Deletes file.

### Output & Logging

- `display(x)`: Prints representation to standard output without newline.
- `println(x)`: Prints followed by newline.
- `newline()`: Prints newline.
- `error(msg)`: Throws runtime error with message.

---

## 2. Standard Library Prelude (`core.sel`)

These functions are defined in `src/core.sel` and automatically available in every SEL program.

### Type Helpers

- `is_even(x)`: Returns `true` if `x % 2 == 0`.
- `is_odd(x)`: Returns `true` if `x % 2 != 0`.
- `is_int(x)`: Checks if `type_of(x) == :int`.
- `is_float(x)`: Checks if `type_of(x) == :float`.
- `is_bool(x)`: Checks if `type_of(x) == :bool`.

### List Processing & Functional Utilities

- `first(list)` / `head(list)`: Returns first element (or `nil` if empty).
- `rest(list)` / `tail(list)`: Returns tail (or `[]` if empty).
- `last(list)`: Returns final element.
- `append(l1, l2)`: Concatenates two lists.
- `map(fn, list)`: Applies `fn` to each element.
- `filter(pred, list)`: Filters elements satisfying `pred`.
- `foldl(fn, acc, list)`: Left fold (reduce).
- `foldr(fn, acc, list)`: Right fold.
- `reverse(list)`: Reverses list elements.

### Pipeline Helpers (Collection-First)

- `map_by(list, fn)`: `list |> map_by(\x -> x * 2)`
- `filter_by(list, pred)`: `list |> filter_by(\x -> is_even(x))`
- `reduce(list, acc, fn)`: `list |> reduce(0, \acc x -> acc + x)`

### Search & Aggregations

- `sum(list)`: Sums numeric list.
- `product(list)`: Multiplies numeric list.
- `contains(list, target)`: Returns `true` if target is in list.
- `find(list, pred)`: Returns first element matching `pred`, or `nil`.
- `any(list, pred)`: Returns `true` if any element matches `pred`.
- `all(list, pred)`: Returns `true` if all elements match `pred`.
- `repeat(fn, n)`: Runs zero-argument function `fn` `n` times.
- `range(limit)`: Sequence `[0, 1, ..., limit - 1]`.
- `range_from(start, limit)`: Sequence `[start, ..., limit - 1]`.
- `range_step(start, limit, step)`: Sequence in increments of `step`.

### Result / Monadic Helpers

- `ok(val)`: Wraps successful value `[:ok, val]`.
- `err(msg)`: Wraps error value `[:err, msg]`.
- `is_ok(result)`: Returns `true` if result is ok.
- `is_err(result)`: Returns `true` if result is err.
- `unwrap(result)`: Extracts value or throws error.
- `unwrap_or(result, default)`: Extracts value or returns default.
- `error_value(result)`: Extracts error message from err container.

### Mathematical Constants

- `pi`: `3.141592653589793`
- `tau`: `6.283185307179586`
- `e`: `2.718281828459045`

### Filesystem Helpers

- `fs_exists(path)`: Checks if file exists.
- `fs_read(path)`: Reads file content as string.
- `fs_write(path, content)`: Writes content to file.
- `fs_list(path)`: Returns list of file names in directory.
- `fs_delete(path)`: Deletes file at path.
