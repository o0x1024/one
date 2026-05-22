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
}
