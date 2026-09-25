use std::process::exit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxMode {
    Auto,
    Alt,
    Sexpr,
}

pub(crate) enum Cli {
    Help,
    Version,
    File(String, SyntaxMode),
    Repl(SyntaxMode),
}

impl Cli {
    pub fn parse() -> Cli {
        let mut args = std::env::args().skip(1);
        let mut syntax_mode = SyntaxMode::Auto;
        let mut file_arg = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Cli::Help,
                "-v" | "--version" => return Cli::Version,
                "--alt" | "-a" => syntax_mode = SyntaxMode::Alt,
                "--sexpr" => syntax_mode = SyntaxMode::Sexpr,
                "-s" | "--syntax" => {
                    if let Some(mode_str) = args.next() {
                        match mode_str.as_str() {
                            "alt" | "sel" => syntax_mode = SyntaxMode::Alt,
                            "sexpr" | "scm" => syntax_mode = SyntaxMode::Sexpr,
                            "auto" => syntax_mode = SyntaxMode::Auto,
                            other => {
                                eprintln!("Unknown syntax mode `{other}`. Expected `alt`, `sexpr`, or `auto`.");
                                exit(1);
                            }
                        }
                    } else {
                        eprintln!("Missing argument for `--syntax`");
                        exit(1);
                    }
                }
                f if f.starts_with('-') => {
                    eprintln!("Unknown flag `{f}`");
                    Cli::help();
                    exit(1);
                }
                _ => {
                    file_arg = Some(arg);
                    break;
                }
            }
        }

        if let Some(file) = file_arg {
            Cli::File(file, syntax_mode)
        } else {
            Cli::Repl(syntax_mode)
        }
    }

    pub fn help() {
        println!("usage: sel [options] [file]");
        println!("Options:");
        println!("  -v | --version         : print sel version");
        println!("  -h | --help            : print this message");
        println!("  -s | --syntax <mode>   : select syntax mode (`alt`, `sexpr`, or `auto`)");
        println!("  -a | --alt             : shorthand for `--syntax alt`");
        println!("Arguments:");
        println!("  file                   : program read from script file (.scm or .sel)");
    }
}
