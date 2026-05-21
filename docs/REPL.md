# Interactive REPL

`sel` provides an interactive Read-Eval-Print Loop (REPL) that features a rich set of command-line utilities for inspecting, loading, and debugging Lisp environments in real time.

Start the REPL by running `sel` without arguments:
```bash
sel
```

## REPL Commands

All REPL-specific directives start with a colon (`:`). The following commands are available:

- **`:help` or `:?`**: Displays the built-in help menu listing all available commands and their descriptions.
- **`:summary`**: Prints a quick count and list of all user-defined bindings currently registered in the active session.
- **`:env [all]`**: Lists detailed representations of environment bindings across lexical scopes. Passing `all` also lists the full set of core standard library built-ins.
- **`:type <expr>`**: Parses and evaluates the provided expression, then prints the resulting value's runtime type (e.g. `Nil`, `Boolean`, `Integer`, `Float`, `String`, `Symbol`, `List`, `Record`, `Closure`, `Macro`, or `Coroutine`).
- **`:load <file-path>`**: Loads, parses, and evaluates an external Scheme file (e.g. `examples/hello.scm`) directly within the current REPL environment, preserving any side effects or definitions.
- **`:clear`**: Clears the terminal screen using standard ANSI escape sequences.
- **`:reset`**: Clears all user-defined variables, functions, and macros from the active environment, resetting it back to the clean default standard library state.
- **`:quit`**: Exits the interactive session and returns to the system shell.

## Line Editing & History
The REPL uses `rustyline` for a modern CLI experience:
- **Command History**: Navigate through previously executed lines using the **Up** and **Down** arrow keys. History is automatically saved in your home directory under `~/.sel_history`.
- **Keyboard Shortcuts**: Supports standard shell line-editing shortcuts like `Ctrl-A` (go to beginning of line), `Ctrl-E` (go to end of line), and `Ctrl-D` / `Ctrl-C` to gracefully terminate.
