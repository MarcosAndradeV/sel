use rustyline::error::ReadlineError;
use std::cell::RefCell;
use std::env;
use std::rc::Rc;

use crate::compiler::parse_all;
use crate::diagnostics::SelError;
use crate::internal::load_core_lib;
use crate::internal::read_script;
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

fn main() {
    match entry() {
        Ok(_) => (),
        Err(e) => println!("{e}"),
    }
}

fn entry() -> Result<(), SelError> {
    let env = Rc::new(RefCell::new(Env::default()));
    env.borrow_mut().parent = Some(load_core_lib());

    let mut args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let script_path = args.remove(1);

        let src = read_script(&script_path)?;

        let asts = parse_all(&src, intern(&script_path.to_string()))?;
        execute_asts(asts, env.clone()).map(|_| ())?;
        Ok(())
    } else {
        repl("sel> ", env)
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
                let line = line.trim();
                _ = rl.add_history_entry(line);
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
                    let asts = parse_all(&src, intern("<test-all-folders>"))
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
                    let asts = parse_all(&src, intern("<test-erros-folder>"))
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
