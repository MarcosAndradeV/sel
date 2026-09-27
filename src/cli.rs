use std::process::exit;

pub(crate) enum Cli {
    Help,
    Version,
    File(String),
    Repl,
}

impl Cli {
    pub fn parse() -> Cli {
        let mut args = std::env::args().skip(1);
        let mut file_arg = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Cli::Help,
                "-v" | "--version" => return Cli::Version,
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
            Cli::File(file)
        } else {
            Cli::Repl
        }
    }

    pub fn help() {
        println!("usage: sel [options] [file]");
        println!("Options:");
        println!("  -v | --version         : print sel version");
        println!("  -h | --help            : print this message");
        println!("Arguments:");
        println!("  file                   : program read from script file (.sel)");
    }
}
