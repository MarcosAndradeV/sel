use rustyline::error::ReadlineError;
use std::cell::RefCell;
use std::env;
use std::rc::Rc;

use crate::cli::Cli;
use crate::diagnostics::SelError;
use crate::internal::load_core_lib;
use crate::internal::read_script;
use crate::internal::value_type_name;
use crate::parser::parse_all;
use crate::runtime::Env;
use crate::runtime::execute_asts;
use crate::types::intern;
use crate::types::lookup;
use crate::value::Value;

mod ast;
mod cli;
mod compiler;
mod diagnostics;
mod internal;
mod lexer;
mod parser;
mod runtime;
mod types;
mod value;

fn main() {
    match entry() {
        Ok(_) => (),
        Err(e) => println!("{e}"),
    }
}

fn entry() -> Result<(), SelError> {
    let cmd = Cli::parse();

    let env = Rc::new(RefCell::new(Env::default()));
    env.borrow_mut().parent = Some(load_core_lib());

    match cmd {
        Cli::Help => {
            Cli::help();
            Ok(())
        }
        Cli::Version => {
            println!("version: {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Cli::File(script_path) => {
            let src = read_script(&script_path)?;
            let asts = parse_all(&src, intern(&script_path.to_string()))?;
            execute_asts(asts, env.clone()).map(|_| ())?;
            Ok(())
        }
        Cli::Repl => repl("sel> ", env),
    }
}

fn repl(prompt: &str, env: Rc<RefCell<Env>>) -> Result<(), SelError> {
    const QUIT_COMMAND: &str = ":quit";
    let repl_file_id = intern("<repl>");
    println!("Welcome to the Sel Scheme repl. (Use `{QUIT_COMMAND}` to exit)");

    let sel_history_path = env::home_dir().unwrap_or_default().join(".sel_history");
    let mut rl =
        rustyline::DefaultEditor::new().expect("cannot create default rustyline::DefaultEditor");
    if rl.load_history(&sel_history_path).is_err() {
        println!("No previous history.");
    }
    loop {
        match rl.readline(prompt) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                _ = rl.add_history_entry(trimmed);
                let line = trimmed;
                if line.starts_with(':') {
                    let mut parts = line.splitn(2, ' ');
                    let cmd = parts.next().unwrap();
                    let arg = parts.next().unwrap_or("").trim();
                    match cmd {
                        QUIT_COMMAND => break,
                        ":help" | ":?" => {
                            println!("Available commands:");
                            println!("  :help, :?         Show this help message");
                            println!("  :summary          Show a summary of user-defined bindings");
                            println!(
                                "  :env [all]        List bindings in the environment (use 'all' to include standard library)"
                            );
                            println!(
                                "  :type <expr>      Evaluate an expression and show its type"
                            );
                            println!(
                                "  :load <file>      Load and execute a Scheme file in the current environment"
                            );
                            println!("  :clear            Clear the screen");
                            println!(
                                "  :reset            Reset the environment (clears user-defined bindings)"
                            );
                            println!("  :quit             Exit the REPL");
                            continue;
                        }
                        ":summary" => {
                            let bindings = env.borrow().bindings.clone();
                            if bindings.is_empty() {
                                println!("Environment is empty. No user-defined bindings.");
                            } else {
                                println!("User-defined bindings ({}):", bindings.len());
                                let mut entries: Vec<_> = bindings
                                    .iter()
                                    .map(|(id, v)| {
                                        let val_str = match v {
                                            Value::Closure(c) => {
                                                let param_names: Vec<String> = c
                                                    .params
                                                    .iter()
                                                    .map(|&pid| lookup(pid))
                                                    .collect();
                                                format!("<closure: ({})>", param_names.join(" "))
                                            }
                                            Value::Macro(m) => {
                                                let param_names: Vec<String> = m
                                                    .params
                                                    .iter()
                                                    .map(|&pid| lookup(pid))
                                                    .collect();
                                                format!("<macro: ({})>", param_names.join(" "))
                                            }
                                            other => other.to_string(),
                                        };
                                        format!(
                                            "  {} := {} ({})",
                                            lookup(*id),
                                            val_str,
                                            value_type_name(v)
                                        )
                                    })
                                    .collect();
                                entries.sort();
                                for e in entries {
                                    println!("{e}");
                                }
                            }
                            continue;
                        }
                        ":env" => {
                            let show_all = arg == "all";
                            let mut current = Some(env.clone());
                            let mut level = 0;
                            while let Some(curr_env) = current {
                                let bindings = curr_env.borrow().bindings.clone();
                                let is_core = level > 0; // standard library or nested parent
                                if is_core && !show_all {
                                    println!(
                                        "[Level {level}: Core Library ({} built-ins)]",
                                        bindings.len()
                                    );
                                    break;
                                } else {
                                    let level_name = if level == 0 {
                                        "REPL".to_string()
                                    } else {
                                        format!("Parent Level {level}")
                                    };
                                    println!(
                                        "[Level {level}: {level_name} ({} bindings)]",
                                        bindings.len()
                                    );
                                    if !bindings.is_empty() {
                                        let mut entries: Vec<_> = bindings
                                            .iter()
                                            .map(|(id, v)| {
                                                let val_str = match v {
                                                    Value::Closure(c) => {
                                                        let param_names: Vec<String> = c
                                                            .params
                                                            .iter()
                                                            .map(|&pid| lookup(pid))
                                                            .collect();
                                                        format!(
                                                            "<closure: ({})>",
                                                            param_names.join(" ")
                                                        )
                                                    }
                                                    Value::Macro(m) => {
                                                        let param_names: Vec<String> = m
                                                            .params
                                                            .iter()
                                                            .map(|&pid| lookup(pid))
                                                            .collect();
                                                        format!(
                                                            "<macro: ({})>",
                                                            param_names.join(" ")
                                                        )
                                                    }
                                                    other => other.to_string(),
                                                };
                                                format!(
                                                    "  {} := {} ({})",
                                                    lookup(*id),
                                                    val_str,
                                                    value_type_name(v)
                                                )
                                            })
                                            .collect();
                                        entries.sort();
                                        for e in entries {
                                            println!("{e}");
                                        }
                                    }
                                }
                                current = curr_env.borrow().parent.clone();
                                level += 1;
                            }
                            continue;
                        }
                        ":type" => {
                            if arg.is_empty() {
                                println!("Usage: :type <expr>");
                                continue;
                            }
                            let asts = match parse_all(arg, repl_file_id) {
                                Ok(asts) => asts,
                                Err(e) => {
                                    println!("{e}");
                                    continue;
                                }
                            };
                            match execute_asts(asts, env.clone()) {
                                Ok(val) => {
                                    println!("{}", value_type_name(&val));
                                }
                                Err(e) => {
                                    println!("{e}");
                                }
                            }
                            continue;
                        }
                        ":load" => {
                            if arg.is_empty() {
                                println!("Usage: :load <file>");
                                continue;
                            }
                            match read_script(arg) {
                                Ok(src) => {
                                    let load_file_id = intern(arg);
                                    match parse_all(&src, load_file_id) {
                                        Ok(asts) => match execute_asts(asts, env.clone()) {
                                            Ok(val) => println!("{val}"),
                                            Err(e) => println!("{e}"),
                                        },
                                        Err(e) => println!("{e}"),
                                    }
                                }
                                Err(e) => {
                                    println!("Error loading file '{arg}': {e}");
                                }
                            }
                            continue;
                        }
                        ":clear" => {
                            print!("\x1B[2J\x1B[1;1H");
                            _ = std::io::Write::flush(&mut std::io::stdout());
                            continue;
                        }
                        ":reset" => {
                            env.borrow_mut().bindings.clear();
                            env.borrow_mut().parent = Some(load_core_lib());
                            println!("Environment reset. User-defined bindings cleared.");
                            continue;
                        }
                        _ => {
                            println!(
                                "Unknown REPL command: `{cmd}`. Type `:help` for available commands."
                            );
                            continue;
                        }
                    }
                }

                let asts = match parse_all(line, repl_file_id) {
                    Ok(asts) => asts,
                    Err(e) => {
                        println!("{e}");
                        continue;
                    }
                };
                let val = match execute_asts(asts, env.clone()) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("{e}");
                        continue;
                    }
                };
                println!("{val}");
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    _ = rl.save_history(&sel_history_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::read_dir;

    use super::*;

    #[test]
    fn test_all_folders() {
        fn test_all_folders_impl() -> Result<(), ()> {
            let tests_dir = read_dir("tests")
                .map_err(|e| eprintln!("Error: cannot read directory tests because {e}"))?;
            for res_entry in tests_dir {
                let entry = res_entry
                    .map_err(|e| eprintln!("Error: cannot read directory tests because {e}"))?;
                if entry
                    .file_type()
                    .map_err(|e| eprintln!("Error: cannot get type of file because {e}"))?
                    .is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "scm")
                {
                    let env = Rc::new(RefCell::new(Env::default()));
                    env.borrow_mut().parent = Some(load_core_lib());
                    println!("TEST: {}", entry.path().display());
                    let epath = entry.path();
                    let src = read_script(&epath).map_err(|e| {
                        eprintln!("Error: cannot read file {} because {e}", epath.display())
                    })?;
                    let asts = parse_all(&src, intern(&epath.to_string_lossy().to_string()))
                        .map_err(|e| eprintln!("{e}"))?;
                    execute_asts(asts, env)
                        .map_err(|e| eprintln!("{}", e))
                        .map(|_| ())?;
                }
            }
            Ok(())
        }

        assert!(test_all_folders_impl().is_ok())
    }

    #[test]
    fn test_errors_folder() {
        fn test_all_folders_impl() -> Result<(), ()> {
            let tests_dir = read_dir("tests/errors")
                .map_err(|e| eprintln!("Error: cannot read directory tests because {e}"))?;
            for res_entry in tests_dir {
                let entry = res_entry
                    .map_err(|e| eprintln!("Error: cannot read directory tests because {e}"))?;
                if entry
                    .file_type()
                    .map_err(|e| eprintln!("Error: cannot get type of file because {e}"))?
                    .is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "scm")
                {
                    let env = Rc::new(RefCell::new(Env::default()));
                    env.borrow_mut().parent = Some(load_core_lib());
                    println!("TEST: {}", entry.path().display());
                    let epath = entry.path();
                    let src = read_script(&epath).map_err(|e| {
                        eprintln!("Error: cannot read file {} because {e}", epath.display())
                    })?;
                    let asts = parse_all(&src, intern(&epath.to_string_lossy().to_string()))
                        .map_err(|e| eprintln!("{e}"))?;
                    execute_asts(asts, env)
                        .map_err(|e| eprintln!("{}", e))
                        .map(|_| ())?;
                }
            }
            Ok(())
        }

        assert!(test_all_folders_impl().is_err())
    }
}
