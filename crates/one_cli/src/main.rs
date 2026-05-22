use one_engine::Engine;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: one <script.js>");
        eprintln!("  or:  one -e '<code>'");
        std::process::exit(1);
    }

    let mut engine = Engine::new();

    if args[1] == "-e" {
        if args.len() < 3 {
            eprintln!("Error: -e requires a code argument");
            std::process::exit(1);
        }
        match engine.eval(&args[2]) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match engine.eval_file(&args[1]) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
