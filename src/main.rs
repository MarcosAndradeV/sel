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
        Cli::Lint(script_path) => {
            run_linter(&script_path, env.clone())?;
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

fn run_linter(script_path: &str, env: Rc<RefCell<Env>>) -> Result<(), SelError> {
    let src = match read_script(script_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file `{script_path}`: {e}");
            std::process::exit(1);
        }
    };
    
    let mut diags = Vec::new();
    let file_id = intern(script_path);
    let asts = crate::parser::parse_all_resilient(&src, file_id, &mut diags);
    
    let mut lint_errors = Vec::new();
    let mut global_functions = std::collections::HashMap::new();
    
    for ast in &asts {
        collect_defines(ast, &mut global_functions);
    }
    
    let mut local_scopes = Vec::new();
    let mut globals = std::collections::HashSet::new();
    for &id in global_functions.keys() {
        globals.insert(id);
    }
    for &id in env.borrow().bindings.keys() {
        globals.insert(id);
    }
    if let Some(parent) = &env.borrow().parent {
        for &id in parent.borrow().bindings.keys() {
            globals.insert(id);
        }
    }
    local_scopes.push(globals);
    
    for ast in &asts {
        check_ast(ast, &mut local_scopes, &global_functions, &mut lint_errors);
    }
    
    let total_syntax_errors = diags.len();
    let total_lint_errors = lint_errors.len();
    
    if total_syntax_errors > 0 || total_lint_errors > 0 {
        if total_syntax_errors > 0 {
            eprintln!("\n=== Syntax Errors ({total_syntax_errors}) ===");
            for err in &diags {
                eprintln!("{err}");
            }
        }
        if total_lint_errors > 0 {
            eprintln!("\n=== Linter Warnings ({total_lint_errors}) ===");
            for err in &lint_errors {
                eprintln!("{err}");
            }
        }
        std::process::exit(1);
    } else {
        println!("No syntax or static analysis issues found in `{script_path}`.");
        Ok(())
    }
}

fn collect_defines(ast: &crate::ast::Ast, global_functions: &mut std::collections::HashMap<u32, (usize, bool)>) {
    match ast {
        crate::ast::Ast::Define(_, id, body) => {
            if let crate::ast::Ast::Lambda(_, params, _) = &**body {
                let mut has_rest = false;
                let mut min_args = 0;
                for p in params {
                    if lookup(*p).starts_with('&') {
                        has_rest = true;
                    } else {
                        min_args += 1;
                    }
                }
                global_functions.insert(*id, (min_args, has_rest));
            }
        }
        crate::ast::Ast::Begin(_, exprs) => {
            for e in exprs {
                collect_defines(e, global_functions);
            }
        }
        _ => {}
    }
}

fn check_ast(
    ast: &crate::ast::Ast,
    scopes: &mut Vec<std::collections::HashSet<u32>>,
    global_functions: &std::collections::HashMap<u32, (usize, bool)>,
    errors: &mut Vec<SelError>,
) {
    match ast {
        crate::ast::Ast::Define(_loc, id, body) => {
            check_ast(body, scopes, global_functions, errors);
            if let Some(globals) = scopes.first_mut() {
                globals.insert(*id);
            }
        }
        crate::ast::Ast::DefMacro(_loc, id, body) => {
            check_ast(body, scopes, global_functions, errors);
            if let Some(globals) = scopes.first_mut() {
                globals.insert(*id);
            }
        }
        crate::ast::Ast::Let(_loc, bindings, body) => {
            let mut let_scope = std::collections::HashSet::new();
            for (id, val) in bindings {
                check_ast(val, scopes, global_functions, errors);
                let_scope.insert(*id);
            }
            scopes.push(let_scope);
            for b in body {
                check_ast(b, scopes, global_functions, errors);
            }
            scopes.pop();
        }
        crate::ast::Ast::Set(loc, id, body) => {
            check_ast(body, scopes, global_functions, errors);
            let mut found = false;
            for s in scopes.iter() {
                if s.contains(id) {
                    found = true;
                    break;
                }
            }
            if !found {
                errors.push(SelError::UnboundVariable(*loc, *id));
            }
        }
        crate::ast::Ast::Lambda(_loc, params, body) => {
            let mut lambda_scope = std::collections::HashSet::new();
            for p in params {
                let name = if lookup(*p).starts_with('&') {
                    &lookup(*p)[1..]
                } else {
                    &lookup(*p)
                };
                lambda_scope.insert(intern(name));
            }
            scopes.push(lambda_scope);
            for b in body {
                check_ast(b, scopes, global_functions, errors);
            }
            scopes.pop();
        }
        crate::ast::Ast::If(_loc, cond, t, f) => {
            check_ast(cond, scopes, global_functions, errors);
            check_ast(t, scopes, global_functions, errors);
            if let Some(f_branch) = f {
                check_ast(f_branch, scopes, global_functions, errors);
            }
        }
        crate::ast::Ast::Begin(_loc, exprs) => {
            for e in exprs {
                check_ast(e, scopes, global_functions, errors);
            }
        }
        crate::ast::Ast::Symbol(loc, id) => {
            let mut found = false;
            for s in scopes.iter() {
                if s.contains(id) {
                    found = true;
                    break;
                }
            }
            if !found {
                let name = lookup(*id);
                if name != "define"
                    && name != "lambda"
                    && name != "let"
                    && name != "if"
                    && name != "begin"
                    && name != "quote"
                    && name != "quasiquote"
                    && name != "unquote"
                    && name != "unquote-splicing"
                    && name != "and"
                    && name != "or"
                    && name != "try"
                    && name != "catch"
                    && name != "co-yield"
                    && name != "co-resume"
                    && name != "nil"
                    && name != "set!"
                    && name != "import"
                    && name != ":private"
                    && name != ":public"
                    && !name.contains('/')
                {
                    errors.push(SelError::UndefinedVariable(*loc, *id));
                }
            }
        }
        crate::ast::Ast::List(_loc, list) => {
            if list.is_empty() {
                return;
            }
            if let crate::ast::Ast::Symbol(s_loc, id) = &list[0] {
                if let Some(&(min_args, has_rest)) = global_functions.get(id) {
                    let actual = list.len() - 1;
                    if has_rest {
                        if actual < min_args {
                            errors.push(SelError::ArityMismatch {
                                loc: *s_loc,
                                expected: min_args,
                                actual,
                            });
                        }
                    } else if actual != min_args {
                        errors.push(SelError::ArityMismatch {
                            loc: *s_loc,
                            expected: min_args,
                            actual,
                        });
                    }
                }
            }
            for e in list {
                check_ast(e, scopes, global_functions, errors);
            }
        }
        crate::ast::Ast::Record(_loc, record) => {
            for (_, v) in record {
                check_ast(v, scopes, global_functions, errors);
            }
        }
        crate::ast::Ast::Try(_loc, body, err_var, catch_body) => {
            check_ast(body, scopes, global_functions, errors);
            let mut catch_scope = std::collections::HashSet::new();
            catch_scope.insert(*err_var);
            scopes.push(catch_scope);
            for b in catch_body {
                check_ast(b, scopes, global_functions, errors);
            }
            scopes.pop();
        }
        crate::ast::Ast::Yield(_loc, val) => {
            check_ast(val, scopes, global_functions, errors);
        }
        crate::ast::Ast::CoResume(_loc, co, arg) => {
            check_ast(co, scopes, global_functions, errors);
            check_ast(arg, scopes, global_functions, errors);
        }
        crate::ast::Ast::Quote(_, _) => {}
        crate::ast::Ast::Quasiquote(_loc, body) => {
            check_ast(body, scopes, global_functions, errors);
        }
        crate::ast::Ast::Unquote(_loc, body) => {
            check_ast(body, scopes, global_functions, errors);
        }
        crate::ast::Ast::UnquoteSplicing(_loc, body) => {
            check_ast(body, scopes, global_functions, errors);
        }
        _ => {}
    }
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
