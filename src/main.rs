use anyhow::Result as AnyhowResult;
use rustyline::error::ReadlineError;
use std::cell::RefCell;
use std::rc::Rc;
use std::{env, fs};

use crate::compiler::read_all;
use crate::runtime::Env;
use crate::runtime::execute_asts;
use crate::types::intern;
use crate::types::lookup;

mod ast;
mod compiler;
mod diagnostics;
mod internal;
mod lexer;
mod runtime;
mod types;

fn main() -> AnyhowResult<()> {
    let env = Rc::new(RefCell::new(Env::default()));
    load_core_lib(env.clone());

    let mut args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let script_path = args.remove(1);

        let mut src = fs::read_to_string(&script_path)?;
        if src.starts_with("#!") {
            if let Some(newline_idx) = src.find('\n') {
                src = src[newline_idx + 1..].to_string();
            } else {
                src = String::new();
            }
        }

        let asts = read_all(&src, intern(&script_path.to_string()))?;
        execute_asts(asts, env)
            .map_err(|e| anyhow::anyhow!("{}", e))
            .map(|_| ())
    } else {
        repl("sel> ", env)
    }
}

fn load_core_lib(env: Rc<RefCell<Env>>) {
    // Load internal library
    internal::load(env.clone());

    // Load core library if exists
    {
        let core_src = include_str!("core.scm");

        match read_all(core_src, intern("<core>")) {
            Ok(asts) => {
                if let Err(e) = execute_asts(asts, env) {
                    eprintln!("Error loading core.scm: {}", e);
                }
            }
            Err(e) => eprintln!("Error parsing core.scm: {}", e),
        }
    }
}

fn repl(prompt: &str, env: Rc<RefCell<Env>>) -> AnyhowResult<()> {
    const QUIT_COMMAND: &str = ":quit";
    let repl_file_id = intern("<repl>");
    println!("Welcome to the Sel Scheme repl. (Use `{QUIT_COMMAND}` to exit)");

    let sel_path = env::home_dir().unwrap_or_default().join(".sel");
    fs::create_dir_all(&sel_path)?;
    let hist_path = sel_path.join("history");
    let mut rl = rustyline::DefaultEditor::new()?;
    if rl.load_history(&hist_path).is_err() {
        println!("No previous history.");
    }
    loop {
        match rl.readline(prompt) {
            Ok(line) => {
                let line = line.trim();
                rl.add_history_entry(line)?;
                match line {
                    "" => continue,
                    QUIT_COMMAND => break,
                    ":summary" => {
                        let mut entries: Vec<_> = env
                            .borrow()
                            .bindings
                            .iter()
                            .map(|(id, v)| format!("{} := {}", lookup(*id), v))
                            .collect();
                        entries.sort();
                        for e in entries {
                            println!("{e}");
                        }
                        continue;
                    }
                    _ => (),
                }

                let asts = match read_all(line, repl_file_id) {
                    Ok(asts) => asts,
                    Err(e) => {
                        println!("Error: {e}");
                        continue;
                    }
                };
                let val = match execute_asts(asts, env.clone()) {
                    Ok(val) => val,
                    Err(e) => {
                        println!("Error: {e}");
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
    rl.save_history(&hist_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::read_dir;

    use super::*;

    #[test]
    fn test_all_folders() {
        fn test_all_folders_impl() -> AnyhowResult<()> {
            let tests_dir = read_dir("tests")?;
            for res_entry in tests_dir {
                let entry = res_entry?;
                if entry.file_type()?.is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "scm")
                {
                    let env = Rc::new(RefCell::new(Env::default()));
                    load_core_lib(env.clone());
                    println!("TEST: {}", entry.path().display());
                    let src = fs::read_to_string(entry.path())?;
                    let asts = read_all(&src, intern("<test-all-folders>"))?;
                    execute_asts(asts, env)
                        .map_err(|e| anyhow::anyhow!("{}", e))
                        .map(|_| ())?;
                }
            }
            Ok(())
        }

        assert!(match test_all_folders_impl() {
            Ok(_) => true,
            Err(e) => {
                println!("{e}");
                false
            }
        })
    }

    #[test]
    fn test_errors_folder() {
        fn test_errors_folder_impl() -> AnyhowResult<()> {
            let tests_dir = read_dir("tests/errors")?;
            for res_entry in tests_dir {
                let entry = res_entry?;
                if entry.file_type()?.is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "scm")
                {
                    let env = Rc::new(RefCell::new(Env::default()));
                    load_core_lib(env.clone());
                    println!("TEST: {}", entry.path().display());
                    let src = fs::read_to_string(entry.path())?;
                    let asts = read_all(&src, intern("<test-errors-folder>"))?;
                    execute_asts(asts, env)
                        .map_err(|e| anyhow::anyhow!("{}", e))
                        .map(|_| ())?;
                }
            }
            Ok(())
        }

        assert!(match test_errors_folder_impl() {
            Ok(_) => false,
            Err(e) => {
                println!("{e}");
                true
            }
        })
    }
}
