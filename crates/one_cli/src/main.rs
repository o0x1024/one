use one_engine::{
    Engine, EngineBuilder, FileModuleResolver, ModuleResolverChain, StaticModuleResolver,
    UrlModuleResolver,
};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        run_repl();
        return;
    }

    match args[1].as_str() {
        "--help" | "-h" => print_help(),
        "--version" | "-V" => print_version(),
        "-e" => {
            if args.len() < 3 {
                eprintln!("Error: -e requires a code argument");
                std::process::exit(1);
            }
            run_code(&args[2]);
        }
        "--repl" => run_repl(),
        path => run_file(path),
    }
}

fn print_help() {
    println!("one - JavaScript Runtime");
    println!();
    println!("USAGE:");
    println!("  one [OPTIONS] [SCRIPT]");
    println!();
    println!("OPTIONS:");
    println!("  -e <code>     Execute inline JavaScript code");
    println!("  --repl        Start interactive REPL");
    println!("  -h, --help    Show this help message");
    println!("  -V, --version Show version information");
    println!();
    println!("EXAMPLES:");
    println!("  one script.js");
    println!("  one -e 'console.log(\"hello\")'");
    println!("  one                          (starts REPL)");
}

fn print_version() {
    println!("one {}", env!("CARGO_PKG_VERSION"));
}

fn run_repl() {
    let mut engine = Engine::new();
    println!("one v{} — JavaScript Runtime", env!("CARGO_PKG_VERSION"));
    println!("Type .exit to quit");
    println!();

    let stdin = io::stdin();
    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() || line.is_empty() {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == ".exit" || line == "exit" {
            break;
        }

        match engine.eval(line) {
            Ok(result) => {
                if !result.is_undefined() {
                    let s = engine.vm().value_to_string(result);
                    println!("{s}");
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
            }
        }
    }
}

fn run_code(code: &str) {
    let mut engine = Engine::new();
    match engine.eval(code) {
        Ok(val) => {
            if !val.is_undefined() {
                println!("{}", engine.vm().value_to_string(val));
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn run_file(path: &str) {
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| Path::new(path).to_path_buf());
    let script_dir = abs_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let source = match std::fs::read_to_string(&abs_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: Failed to read file '{path}': {e}");
            std::process::exit(1);
        }
    };

    let has_imports = source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("import ") || trimmed.starts_with("import{")
    });

    let chain = ModuleResolverChain::new()
        .push(StaticModuleResolver::new())
        .push(FileModuleResolver::new(&script_dir))
        .push(UrlModuleResolver::with_default_cache());

    let mut engine = EngineBuilder::new()
        .module_resolver(chain)
        .build();

    if has_imports {
        match engine.eval_module(&source, &abs_path.to_string_lossy()) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match engine.eval_file(path) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
