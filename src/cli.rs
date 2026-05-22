use std::process::exit;

pub(crate) enum Cli {
    Help,
    Version,
    File(String),
    Repl,
    Lint(String),
}

impl Cli {
    pub fn parse() -> Cli {
        let mut args = std::env::args().skip(1);
        let Some(first) = args.next() else {
            return Cli::Repl;
        };
        match first.as_str() {
            "-h" | "--help" => Cli::Help,
            "-v" | "--version" => Cli::Version,
            "--lint" => {
                let Some(file) = args.next() else {
                    eprintln!("Error: `--lint` requires a file path argument.");
                    exit(1);
                };
                Cli::Lint(file)
            }
            f if f.starts_with("-") => {
                eprintln!("Unknow flag `{f}`");
                Cli::help();
                exit(1);
            }
            _ => Cli::File(first),
        }
    }

    pub fn help() {
        println!("usage: sel [option] [file]");
        println!("Options:");
        println!("-v | --version: print sel version");
        println!("-h | --help   : print this message");
        println!("--lint        : run static linter checks on a file");
        println!("Arguments:");
        println!("file: program read from script file");
    }
}
