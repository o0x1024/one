pub mod convert;
pub mod object;
pub mod shape;
pub mod vm;

pub use convert::{FromJsValue, IntoJsValue};
pub use object::{FunctionObject, JsObject, ObjectKind, PromiseState, Property};
pub use shape::{PropertyAttributes, Shape};
pub use vm::{ExecutionHook, Vm};

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

    #[test]
    fn object_literal() {
        let result = run(r#"let obj = {x: 1, y: 2}; return obj.x;"#);
        assert_eq!(result.as_i32(), Some(1));
    }

    #[test]
    fn object_property_set() {
        let result = run(r#"let obj = {}; obj.x = 42; return obj.x;"#);
        assert_eq!(result.as_i32(), Some(42));
    }

    #[test]
    fn array_literal() {
        let result = run(r#"let arr = [10, 20, 30]; return arr;"#);
        assert!(result.is_object());
    }

    #[test]
    fn nested_property_access() {
        let result = run(r#"let a = {b: {c: 99}}; return a.b.c;"#);
        assert_eq!(result.as_i32(), Some(99));
    }

    #[test]
    fn call_js_function() {
        let result = run("function add(a, b) { return a + b; } return add(3, 4);");
        assert!(result.to_number() == 7.0);
    }

    #[test]
    fn function_no_return() {
        let result = run("function noop() {} return noop();");
        assert!(result.is_undefined());
    }

    #[test]
    fn function_with_locals() {
        let result = run("function f(x) { let y = x * 2; return y + 1; } return f(10);");
        assert!(result.to_number() == 21.0);
    }

    #[test]
    fn nested_function_calls() {
        let result = run(
            "function double(x) { return x * 2; } function quad(x) { return double(double(x)); } return quad(3);",
        );
        assert!(result.to_number() == 12.0);
    }

    #[test]
    fn function_expression() {
        let result = run("let add = function(a, b) { return a + b; }; return add(5, 6);");
        assert!(result.to_number() == 11.0);
    }

    #[test]
    fn arrow_function() {
        let result = run("let sq = (x) => { return x * x; }; return sq(7);");
        assert!(result.to_number() == 49.0);
    }

    #[test]
    fn function_as_argument() {
        let result = run(
            "function apply(f, x) { return f(x); } function double(x) { return x * 2; } return apply(double, 5);",
        );
        assert!(result.to_number() == 10.0);
    }

    #[test]
    fn recursive_function() {
        let result = run(
            "function fib(n) { if (n <= 1) { return n; } return fib(n - 1) + fib(n - 2); } return fib(10);",
        );
        assert!(result.to_number() == 55.0);
    }

    #[test]
    fn new_operator() {
        let result = run("function Point(x, y) { this.x = x; this.y = y; } let p = new Point(3, 4); return p.x;");
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn new_operator_y() {
        let result = run("function Point(x, y) { this.x = x; this.y = y; } let p = new Point(3, 4); return p.y;");
        assert!(result.to_number() == 4.0);
    }

    #[test]
    fn prototype_method() {
        let result = run(r#"
            function Point(x, y) { this.x = x; this.y = y; }
            Point.prototype = {};
            Point.prototype.sum = function() { return this.x + this.y; };
            let p = new Point(3, 4);
            return p.sum();
        "#);
        assert!(result.to_number() == 7.0);
    }

    #[test]
    fn class_basic() {
        let result = run(r#"
            class Point {
                constructor(x, y) { this.x = x; this.y = y; }
            }
            let p = new Point(10, 20);
            return p.x;
        "#);
        assert!(result.to_number() == 10.0);
    }

    #[test]
    fn class_with_method() {
        let result = run(r#"
            class Calc {
                constructor(v) { this.v = v; }
                double() { return this.v * 2; }
            }
            let c = new Calc(21);
            return c.double();
        "#);
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn constructor_returns_undefined_gives_this() {
        let result = run("function Foo() { this.x = 99; } let f = new Foo(); return f.x;");
        assert!(result.to_number() == 99.0);
    }

    #[test]
    fn try_catch_basic() {
        let result = run(
            r#"
            let x = 0;
            try {
                throw 42;
            } catch (e) {
                x = e;
            }
            return x;
        "#,
        );
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn try_catch_no_throw() {
        let result = run(
            r#"
            let x = 1;
            try {
                x = 2;
            } catch (e) {
                x = 3;
            }
            return x;
        "#,
        );
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn try_catch_string() {
        let result = run(
            r#"
            let msg = "";
            try {
                throw "error!";
            } catch (e) {
                msg = e;
            }
            return msg;
        "#,
        );
        assert!(result.is_string());
    }

    #[test]
    fn try_finally() {
        let result = run(
            r#"
            let x = 0;
            try {
                x = 1;
            } finally {
                x = x + 10;
            }
            return x;
        "#,
        );
        assert!(result.to_number() == 11.0);
    }

    #[test]
    fn try_catch_finally() {
        let result = run(
            r#"
            let x = 0;
            try {
                throw 1;
            } catch (e) {
                x = e;
            } finally {
                x = x + 100;
            }
            return x;
        "#,
        );
        assert!(result.to_number() == 101.0);
    }

    #[test]
    fn nested_try_catch() {
        let result = run(
            r#"
            let x = 0;
            try {
                try {
                    throw 1;
                } catch (e) {
                    x = e;
                    throw 2;
                }
            } catch (e) {
                x = x + e;
            }
            return x;
        "#,
        );
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn uncaught_exception_returns_error() {
        let program = Parser::parse("throw 42;").unwrap();
        let code = Compiler::compile(&program);
        let mut vm = Vm::new();
        let result = vm.execute(&code);
        assert!(result.is_err());
    }

    #[test]
    fn try_catch_in_function() {
        let result = run(
            r#"
            function safe_div(a, b) {
                try {
                    if (b === 0) { throw "division by zero"; }
                    return a / b;
                } catch (e) {
                    return -1;
                }
            }
            return safe_div(10, 0);
        "#,
        );
        assert!(result.to_number() == -1.0);
    }

    #[test]
    fn array_element_access() {
        let result = run("let arr = [10, 20, 30]; return arr[1];");
        assert!(result.to_number() == 20.0);
    }

    #[test]
    fn array_destructuring() {
        let result = run("let arr = [1, 2, 3]; let [a, b, c] = arr; return b;");
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn object_destructuring() {
        let result = run(r#"let obj = {x: 10, y: 20}; let {x, y} = obj; return x + y;"#);
        assert!(result.to_number() == 30.0);
    }

    #[test]
    fn for_of_array() {
        let result = run(
            r#"
            let sum = 0;
            let arr = [1, 2, 3, 4, 5];
            for (let x of arr) {
                sum = sum + x;
            }
            return sum;
        "#,
        );
        assert!(result.to_number() == 15.0);
    }

    #[test]
    fn computed_property_set() {
        let result = run(
            r#"
            let obj = {};
            obj["key"] = 42;
            return obj["key"];
        "#,
        );
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn array_push_read() {
        let result = run(
            r#"
            let arr = [10, 20];
            return arr[0] + arr[1];
        "#,
        );
        assert!(result.to_number() == 30.0);
    }

    #[test]
    fn shape_object_operations() {
        let result = run("let o = {a: 1, b: 2, c: 3}; return o.a + o.b + o.c;");
        assert!(result.to_number() == 6.0);
    }

    #[test]
    fn shape_property_overwrite() {
        let result = run("let o = {x: 1}; o.x = 42; return o.x;");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn shape_object_keys_with_shapes() {
        let mut obj = JsObject::new();
        obj.set_property("a".to_string(), JsValue::from_i32(1));
        obj.set_property("b".to_string(), JsValue::from_i32(2));
        assert_eq!(obj.enumerable_keys().len(), 2);
    }

    #[test]
    fn constant_folding() {
        let result = run("return 2 + 3;");
        assert!(result.to_number() == 5.0);
    }

    #[test]
    fn constant_folding_mul() {
        let result = run("return 6 * 7;");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn inc_dec_pattern() {
        let result = run("let x = 0; x = x + 1; x = x + 1; x = x + 1; return x;");
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn dec_pattern() {
        let result = run("let x = 10; x = x - 1; return x;");
        assert!(result.to_number() == 9.0);
    }

    #[test]
    fn loop_counter_optimized() {
        let result = run(
            "let sum = 0; let i = 0; while(i < 1000) { sum = sum + i; i = i + 1; } return sum;",
        );
        assert!(result.to_number() == 499500.0);
    }

    #[test]
    fn not_jump_fusion() {
        let result = run("let x = true; if (!x) { return 1; } return 2;");
        assert!(result.to_number() == 2.0);
    }
}
