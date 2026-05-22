use one_compiler::Compiler;
use one_core::{CompileError, JsValue, OneError, OneResult};
use one_parser::parser::Parser;
use one_vm::Vm;

pub struct Engine {
    vm: Vm,
}

impl Engine {
    pub fn new() -> Self {
        let mut vm = Vm::new();
        one_runtime::install_builtins(&mut vm);
        Engine { vm }
    }

    /// Execute JavaScript source code
    pub fn eval(&mut self, source: &str) -> OneResult<JsValue> {
        let program = Parser::parse(source).map_err(|e| {
            OneError::CompileError(CompileError {
                message: e.message,
                file: Some("<eval>".into()),
                line: 0,
                column: 0,
            })
        })?;
        let code = Compiler::compile(&program);
        self.vm.execute(&code)
    }

    /// Execute JavaScript from a file path
    pub fn eval_file(&mut self, path: &str) -> OneResult<JsValue> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            OneError::InternalError(format!("Failed to read file '{path}': {e}"))
        })?;
        let program = Parser::parse(&source).map_err(|e| {
            OneError::CompileError(CompileError {
                message: e.message,
                file: Some(path.into()),
                line: 0,
                column: 0,
            })
        })?;
        let code = Compiler::compile(&program);
        self.vm.execute(&code)
    }

    /// Get the underlying VM for advanced operations
    pub fn vm(&self) -> &Vm {
        &self.vm
    }

    /// Get the underlying VM mutably
    pub fn vm_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_eval_number() {
        let mut engine = Engine::new();
        let result = engine.eval("return 42;").unwrap();
        assert_eq!(result.as_i32(), Some(42));
    }

    #[test]
    fn engine_eval_string() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "hello";"#);
        assert!(result.is_ok());
    }

    #[test]
    fn engine_eval_arithmetic() {
        let mut engine = Engine::new();
        let result = engine.eval("return 2 + 3 * 4;").unwrap();
        assert!(result.to_number() == 14.0);
    }

    #[test]
    fn engine_eval_variable() {
        let mut engine = Engine::new();
        let result = engine.eval("let x = 10; let y = 20; return x + y;").unwrap();
        assert!(result.to_number() == 30.0);
    }

    #[test]
    fn engine_eval_console_log() {
        // This should not panic — console.log is installed
        let mut engine = Engine::new();
        let result = engine.eval(r#"console.log("Hello World");"#);
        assert!(result.is_ok());
    }

    #[test]
    fn engine_eval_parse_error() {
        let mut engine = Engine::new();
        let result = engine.eval("let = ;");
        assert!(result.is_err());
    }

    #[test]
    fn engine_default() {
        let engine = Engine::default();
        assert!(engine.vm().get_global("console").is_object());
    }

    #[test]
    fn console_log_still_works_with_functions() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"function greet(name) { console.log("Hello " + name); } greet("World");"#);
        assert!(result.is_ok());
    }

    fn run(src: &str) -> JsValue {
        let mut engine = Engine::new();
        engine.eval(src).expect("execution failed")
    }

    fn run_with_gc_threshold(src: &str, threshold: usize) -> JsValue {
        let mut engine = Engine::new();
        engine.vm_mut().set_gc_threshold(threshold);
        engine.eval(src).expect("execution failed")
    }

    #[test]
    fn promise_resolve() {
        let result = run(
            r#"
            result = 0;
            let p = Promise.resolve(42);
            p.then(function(v) { result = v; });
            return result;
        "#,
        );
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn promise_reject_catch() {
        let result = run(
            r#"
            result = 0;
            let p = Promise.reject("error");
            p.catch(function(e) { result = e; });
            return result;
        "#,
        );
        assert!(result.is_string());
    }

    #[test]
    fn promise_then_chain() {
        let result = run(
            r#"
            result = 0;
            Promise.resolve(10).then(function(v) {
                result = v * 2;
            });
            return result;
        "#,
        );
        assert!(result.to_number() == 20.0);
    }

    #[test]
    fn promise_constructor() {
        let result = run(
            r#"
            result = 0;
            let p = new Promise(function(resolve, reject) {
                resolve(99);
            });
            p.then(function(v) { result = v; });
            return result;
        "#,
        );
        assert!(result.to_number() == 99.0);
    }

    #[test]
    fn eval_basic() {
        let result = run("return eval('1 + 2');");
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn eval_variable_access() {
        let result = run("let x = 10; return eval('x + 5');");
        assert!(result.to_number() == 15.0);
    }

    #[test]
    fn eval_string() {
        let result = run(r#"return eval('"hello"');"#);
        assert!(result.is_string());
    }

    #[test]
    fn eval_non_string_passthrough() {
        let result = run("return eval(42);");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn use_strict_detection() {
        let result = run(r#""use strict"; return 42;"#);
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn object_keys() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let obj = {a: 1, b: 2, c: 3}; return Object.keys(obj).length;")
            .unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn object_values() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let obj = {x: 10, y: 20}; let vals = Object.values(obj); return vals[0] + vals[1];")
            .unwrap();
        assert!(result.to_number() == 30.0);
    }

    #[test]
    fn object_assign() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let a = {x: 1}; let b = {y: 2}; Object.assign(a, b); return a.y;")
            .unwrap();
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn object_create() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let proto = {greet: function() { return 42; }};
                let obj = Object.create(proto);
                return obj.greet();
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn object_freeze() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let obj = {x: 10}; Object.freeze(obj); obj.x = 99; return obj.x;")
            .unwrap();
        assert!(result.to_number() == 10.0);
    }

    #[test]
    fn object_has_own() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let obj = {a: 1}; return Object.hasOwn(obj, 'a');")
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn gc_survives_collection() {
        let result = run_with_gc_threshold(
            r#"
            let last = 0;
            let i = 0;
            while (i < 100) {
                let obj = {value: i};
                last = obj.value;
                i = i + 1;
            }
            return last;
        "#,
            4096,
        );
        assert!(result.to_number() == 99.0);
    }

    #[test]
    fn array_push() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let arr = [1, 2]; arr.push(3); return arr.length;")
            .unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn array_pop() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let arr = [1, 2, 3]; let x = arr.pop(); return x;")
            .unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn array_map() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let arr = [1, 2, 3];
                let doubled = arr.map(function(x) { return x * 2; });
                return doubled[0] + doubled[1] + doubled[2];
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 12.0);
    }

    #[test]
    fn array_filter() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let arr = [1, 2, 3, 4, 5];
                let evens = arr.filter(function(x) { return x % 2 === 0; });
                return evens.length;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn array_reduce() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let arr = [1, 2, 3, 4, 5];
                let sum = arr.reduce(function(acc, x) { return acc + x; }, 0);
                return sum;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 15.0);
    }

    #[test]
    fn array_foreach() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let sum = 0;
                let arr = [10, 20, 30];
                arr.forEach(function(x) { sum = sum + x; });
                return sum;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 60.0);
    }

    #[test]
    fn array_indexof() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let arr = [10, 20, 30]; return arr.indexOf(20);")
            .unwrap();
        assert!(result.to_number() == 1.0);
    }

    #[test]
    fn array_join() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let arr = [1, 2, 3]; return arr.join("-");"#)
            .unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn array_includes() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let arr = [1, 2, 3]; return arr.includes(2);")
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn array_is_array() {
        let mut engine = Engine::new();
        let result = engine
            .eval("return Array.isArray([1, 2, 3]);")
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn array_slice() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let arr = [1, 2, 3, 4, 5]; let s = arr.slice(1, 3); return s.length;")
            .unwrap();
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn array_concat() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let a = [1, 2]; let b = [3, 4]; let c = a.concat(b); return c.length;")
            .unwrap();
        assert!(result.to_number() == 4.0);
    }

    #[test]
    fn array_find() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let arr = [1, 2, 3, 4, 5];
                let found = arr.find(function(x) { return x > 3; });
                return found;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 4.0);
    }

    #[test]
    fn array_reverse() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let arr = [1, 2, 3]; arr.reverse(); return arr[0];")
            .unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn string_length() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "hello".length;"#).unwrap();
        assert!(result.to_number() == 5.0);
    }

    #[test]
    fn string_to_upper() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "hello".toUpperCase();"#).unwrap();
        assert!(result.is_string());
        assert_eq!(engine.vm().value_to_string(result), "HELLO");
    }

    #[test]
    fn string_includes() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"return "hello world".includes("world");"#)
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn string_split() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "a,b,c".split(",").length;"#).unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn string_trim() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "  hello  ".trim();"#).unwrap();
        assert_eq!(engine.vm().value_to_string(result), "hello");
    }

    #[test]
    fn string_indexof() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "hello".indexOf("ll");"#).unwrap();
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn string_slice() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "hello".slice(1, 3);"#).unwrap();
        assert_eq!(engine.vm().value_to_string(result), "el");
    }

    #[test]
    fn string_replace() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return "hello".replace("l", "r");"#).unwrap();
        assert_eq!(engine.vm().value_to_string(result), "herlo");
    }

    #[test]
    fn number_is_nan() {
        let mut engine = Engine::new();
        let result = engine.eval("return Number.isNaN(0 / 0);").unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn number_is_integer() {
        let mut engine = Engine::new();
        let result = engine.eval("return Number.isInteger(42);").unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn number_parse_int() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return Number.parseInt("42");"#).unwrap();
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn math_floor() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.floor(3.7);").unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn math_ceil() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.ceil(3.2);").unwrap();
        assert!(result.to_number() == 4.0);
    }

    #[test]
    fn math_round() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.round(3.5);").unwrap();
        assert!(result.to_number() == 4.0);
    }

    #[test]
    fn math_abs() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.abs(-42);").unwrap();
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn math_max() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.max(1, 5, 3);").unwrap();
        assert!(result.to_number() == 5.0);
    }

    #[test]
    fn math_min() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.min(1, 5, 3);").unwrap();
        assert!(result.to_number() == 1.0);
    }

    #[test]
    fn math_sqrt() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.sqrt(16);").unwrap();
        assert!(result.to_number() == 4.0);
    }

    #[test]
    fn math_pi() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.PI;").unwrap();
        assert!((result.to_number() - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn math_pow() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.pow(2, 10);").unwrap();
        assert!(result.to_number() == 1024.0);
    }

    #[test]
    fn math_random() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let r = Math.random(); return r >= 0 && r < 1;")
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn math_trunc() {
        let mut engine = Engine::new();
        let result = engine.eval("return Math.trunc(3.9);").unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn json_stringify_number() {
        let mut engine = Engine::new();
        let result = engine.eval("return JSON.stringify(42);").unwrap();
        assert_eq!(engine.vm().value_to_string(result), "42");
    }

    #[test]
    fn json_stringify_string() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return JSON.stringify("hello");"#).unwrap();
        assert_eq!(engine.vm().value_to_string(result), r#""hello""#);
    }

    #[test]
    fn json_stringify_object() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"return JSON.stringify({a: 1, b: 2});"#)
            .unwrap();
        let s = engine.vm().value_to_string(result);
        assert!(s.contains("\"a\":1"));
        assert!(s.contains("\"b\":2"));
    }

    #[test]
    fn json_stringify_array() {
        let mut engine = Engine::new();
        let result = engine.eval("return JSON.stringify([1, 2, 3]);").unwrap();
        assert_eq!(engine.vm().value_to_string(result), "[1,2,3]");
    }

    #[test]
    fn json_parse_number() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return JSON.parse("42");"#).unwrap();
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn json_parse_object() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let obj = JSON.parse('{"x":1,"y":2}'); return obj.x + obj.y;"#)
            .unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn json_parse_array() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let arr = JSON.parse("[1,2,3]"); return arr.length;"#)
            .unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn error_constructor() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let e = new Error("test"); return e.message;"#)
            .unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn error_name() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let e = new TypeError("bad type"); return e.name;"#)
            .unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn throw_error_object() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                try {
                    throw new Error("oops");
                } catch (e) {
                    return e.message;
                }
            "#,
            )
            .unwrap();
        assert!(result.is_string());
    }
}
