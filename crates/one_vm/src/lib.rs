pub mod vm;

pub use vm::Vm;

#[cfg(test)]
mod tests {
    use super::*;
    use one_compiler::Compiler;
    use one_core::JsValue;
    use one_parser::parser::Parser;

    fn run(src: &str) -> JsValue {
        let program = Parser::parse(src).expect("parse failed");
        let code = Compiler::compile(&program);
        let mut vm = Vm::new();
        vm.execute(&code).expect("execution failed")
    }

    #[test]
    fn execute_number() {
        let result = run("return 42;");
        assert_eq!(result.as_i32(), Some(42));
    }

    #[test]
    fn execute_addition() {
        let result = run("return 1 + 2;");
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn execute_subtraction() {
        let result = run("return 10 - 3;");
        assert!(result.to_number() == 7.0);
    }

    #[test]
    fn execute_multiplication() {
        let result = run("return 6 * 7;");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn execute_boolean_true() {
        let result = run("return true;");
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn execute_null() {
        let result = run("return null;");
        assert!(result.is_null());
    }

    #[test]
    fn execute_variable() {
        let result = run("let x = 42; return x;");
        assert_eq!(result.as_i32(), Some(42));
    }

    #[test]
    fn execute_variable_arithmetic() {
        let result = run("let a = 10; let b = 20; return a + b;");
        assert!(result.to_number() == 30.0);
    }

    #[test]
    fn execute_comparison() {
        let result = run("return 5 < 10;");
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn execute_host_function() {
        let program = Parser::parse("return add(1, 2);").expect("parse failed");
        let code = Compiler::compile(&program);
        let mut vm = Vm::new();
        vm.register_host_fn("add", |_vm, args| {
            let a = args[0].to_number();
            let b = args[1].to_number();
            Ok(JsValue::from_f64(a + b))
        });
        let result = vm.execute(&code).expect("failed");
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn execute_console_log() {
        use std::sync::{Arc, Mutex};
        let output = Arc::new(Mutex::new(Vec::<String>::new()));
        let output_clone = output.clone();

        let program = Parser::parse(r#"console.log("Hello World");"#).expect("parse failed");
        let code = Compiler::compile(&program);
        let mut vm = Vm::new();
        vm.register_host_fn("console.log", move |vm, args| {
            let mut out = output_clone.lock().unwrap();
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    out.push(" ".into());
                }
                out.push(vm.value_to_string(*arg));
            }
            Ok(JsValue::undefined())
        });
        vm.execute(&code).expect("failed");

        let out = output.lock().unwrap();
        assert_eq!(out.join(""), "Hello World");
    }

    #[test]
    fn execute_negation() {
        let result = run("let x = 5; return -x;");
        assert!(result.to_number() == -5.0);
    }

    #[test]
    fn execute_not() {
        let result = run("return !true;");
        assert_eq!(result.as_bool(), Some(false));
    }

    #[test]
    fn vm_default_returns_undefined() {
        let result = run("42;");
        assert!(result.is_undefined());
    }
}
