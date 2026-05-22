use std::collections::HashMap;

use one_compiler::{Compiler, ImportSpec, ModuleExport};
use one_core::{CompileError, JsValue, OneError, OneResult};
use one_parser::parser::Parser;
use one_vm::Vm;

pub struct Engine {
    vm: Vm,
    registered_modules: HashMap<String, String>,
    module_cache: HashMap<String, HashMap<String, JsValue>>,
    baseline_globals: HashMap<String, JsValue>,
}

impl Engine {
    pub fn new() -> Self {
        let mut vm = Vm::new();
        one_runtime::install_builtins(&mut vm);
        let baseline_globals = vm.snapshot_globals();
        Engine {
            vm,
            registered_modules: HashMap::new(),
            module_cache: HashMap::new(),
            baseline_globals,
        }
    }

    /// Register a virtual module by specifier name.
    pub fn register_module(&mut self, name: &str, source: &str) {
        self.registered_modules
            .insert(name.to_string(), source.to_string());
    }

    pub fn run_event_loop(&mut self) -> OneResult<()> {
        self.vm.run_event_loop()
    }

    pub fn eval_async(&mut self, source: &str) -> OneResult<JsValue> {
        let result = self.eval(source)?;
        self.run_event_loop()?;
        Ok(result)
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

    /// Execute an ES module.
    pub fn eval_module(&mut self, source: &str, path: &str) -> OneResult<JsValue> {
        let program = Parser::parse_module(source).map_err(|e| {
            OneError::CompileError(CompileError {
                message: e.message,
                file: Some(path.into()),
                line: 0,
                column: 0,
            })
        })?;
        let code = Compiler::compile_module(&program);

        if let Some(module_info) = &code.module_info {
            for import in &module_info.imports {
                let exports = self.load_module(&import.source, path)?;
                self.apply_imports(import, &exports);
            }
        }

        self.vm.execute(&code)
    }

    fn load_module(
        &mut self,
        specifier: &str,
        _referrer: &str,
    ) -> OneResult<HashMap<String, JsValue>> {
        if let Some(exports) = self.module_cache.get(specifier) {
            return Ok(exports.clone());
        }

        let source = self.registered_modules.get(specifier).ok_or_else(|| {
            OneError::InternalError(format!("Module not found: {specifier}"))
        })?;

        let saved_globals = self.vm.snapshot_globals();
        self.vm.restore_globals(self.baseline_globals.clone());

        let program = Parser::parse_module(source).map_err(|e| {
            OneError::CompileError(CompileError {
                message: e.message,
                file: Some(specifier.into()),
                line: 0,
                column: 0,
            })
        })?;
        let module_code = Compiler::compile_module(&program);

        if let Some(module_info) = &module_code.module_info {
            for import in &module_info.imports {
                let exports = self.load_module(&import.source, specifier)?;
                self.apply_imports(import, &exports);
            }
        }

        self.vm.execute(&module_code)?;

        let export_specs = module_code
            .module_info
            .as_ref()
            .map(|info| info.exports.as_slice())
            .unwrap_or(&[]);
        let exports = self.collect_exports(export_specs);
        self.module_cache
            .insert(specifier.to_string(), exports.clone());

        self.vm.restore_globals(saved_globals);
        Ok(exports)
    }

    fn apply_imports(
        &mut self,
        import: &one_compiler::ModuleImport,
        exports: &HashMap<String, JsValue>,
    ) {
        for spec in &import.specifiers {
            match spec {
                ImportSpec::Default(local) => {
                    if let Some(val) = exports.get("default") {
                        self.vm.set_global(local, *val);
                    }
                }
                ImportSpec::Named { local, imported } => {
                    if let Some(val) = exports.get(imported) {
                        self.vm.set_global(local, *val);
                    }
                }
                ImportSpec::Namespace(local) => {
                    let ns = self.create_namespace_object(exports);
                    self.vm.set_global(local, ns);
                }
            }
        }
    }

    fn collect_exports(&self, export_specs: &[ModuleExport]) -> HashMap<String, JsValue> {
        let mut exports = HashMap::new();
        for spec in export_specs {
            let val = self.vm.get_global(&spec.local);
            exports.insert(spec.exported.clone(), val);
        }
        exports
    }

    fn create_namespace_object(&mut self, exports: &HashMap<String, JsValue>) -> JsValue {
        let pairs: Vec<(String, JsValue)> = exports
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        self.vm.create_object_from_pairs(&pairs)
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

    #[test]
    fn map_basic() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let m = new Map();
                m.set("a", 1);
                m.set("b", 2);
                return m.get("a");
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 1.0);
    }

    #[test]
    fn map_size() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let m = new Map();
                m.set("x", 10);
                m.set("y", 20);
                return m.size;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn map_has() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let m = new Map();
                m.set("key", "value");
                return m.has("key");
            "#,
            )
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn map_delete() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let m = new Map();
                m.set("a", 1);
                m.delete("a");
                return m.has("a");
            "#,
            )
            .unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }

    #[test]
    fn map_foreach() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let m = new Map();
                m.set("a", 10);
                m.set("b", 20);
                let sum = 0;
                m.forEach(function(v, k) { sum = sum + v; });
                return sum;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 30.0);
    }

    #[test]
    fn set_basic() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let s = new Set();
                s.add(1);
                s.add(2);
                s.add(2);
                return s.size;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 2.0);
    }

    #[test]
    fn set_has() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let s = new Set();
                s.add(42);
                return s.has(42);
            "#,
            )
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn set_delete() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let s = new Set();
                s.add(1);
                s.add(2);
                s.delete(1);
                return s.size;
            "#,
            )
            .unwrap();
        assert!(result.to_number() == 1.0);
    }

    #[test]
    fn date_now() {
        let mut engine = Engine::new();
        let result = engine.eval("return Date.now() > 0;").unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn date_constructor() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let d = new Date(); return d.getTime() > 0;")
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn date_from_ms() {
        let mut engine = Engine::new();
        let result = engine.eval("let d = new Date(0); return d.getTime();").unwrap();
        assert!(result.to_number() == 0.0);
    }

    #[test]
    fn date_get_full_year() {
        let mut engine = Engine::new();
        let result = engine
            .eval("let d = new Date(1700000000000); return d.getFullYear();")
            .unwrap();
        assert!(result.to_number() == 2023.0);
    }

    #[test]
    fn date_to_iso_string() {
        let mut engine = Engine::new();
        let result = engine.eval("let d = new Date(0); return d.toISOString();").unwrap();
        assert!(result.is_string());
        assert!(
            engine.vm().value_to_string(result).starts_with("1970-01-01"),
            "expected ISO string starting with 1970-01-01"
        );
    }

    #[test]
    fn symbol_basic() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let s = Symbol("test"); return typeof s;"#)
            .unwrap();
        assert!(result.is_string());
        assert_eq!(engine.vm().value_to_string(result), "symbol");
    }

    #[test]
    fn symbol_unique() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let a = Symbol("x"); let b = Symbol("x"); return a === b;"#)
            .unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }

    #[test]
    fn symbol_for_shared() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let a = Symbol.for("shared");
                let b = Symbol.for("shared");
                return a === b;
            "#,
            )
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn regexp_test() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let re = new RegExp("hello");
                return re.test("hello world");
            "#,
            )
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn regexp_test_fail() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let re = new RegExp("xyz");
                return re.test("hello world");
            "#,
            )
            .unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }

    #[test]
    fn regexp_digit() {
        let mut engine = Engine::new();
        let result = engine
            .eval(
                r#"
                let re = new RegExp("\\d+");
                return re.test("abc123");
            "#,
            )
            .unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn module_export_import() {
        let mut engine = Engine::new();
        engine.register_module(
            "math_utils",
            r#"
            export let PI = 3.14159;
            export function double(x) { return x * 2; }
        "#,
        );

        let result = engine
            .eval_module(
                r#"
            import { PI, double } from "math_utils";
            return PI + double(5);
        "#,
                "<test>",
            )
            .unwrap();
        assert!((result.to_number() - 13.14159).abs() < 0.001);
    }

    #[test]
    fn module_default_export() {
        let mut engine = Engine::new();
        engine.register_module(
            "greeter",
            r#"
            export default function() { return 42; }
        "#,
        );

        let result = engine
            .eval_module(
                r#"
            import greet from "greeter";
            return greet();
        "#,
                "<test>",
            )
            .unwrap();
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn module_caching() {
        let mut engine = Engine::new();
        engine.register_module(
            "counter",
            r#"
            export let count = 0;
            count = count + 1;
        "#,
        );

        let r1 = engine
            .eval_module(
                r#"import { count } from "counter"; return count;"#,
                "<test>",
            )
            .unwrap();
        let r2 = engine
            .eval_module(
                r#"import { count } from "counter"; return count;"#,
                "<test2>",
            )
            .unwrap();
        assert!(r1.to_number() == 1.0);
        assert!(r2.to_number() == 1.0);
    }

    #[test]
    fn arrow_function_basic() {
        let result = run("let add = (a, b) => a + b; return add(3, 4);");
        assert!(result.to_number() == 7.0);
    }

    #[test]
    fn arrow_function_single_param() {
        let result = run("let double = x => x * 2; return double(5);");
        assert!(result.to_number() == 10.0);
    }

    #[test]
    fn arrow_function_body() {
        let result = run(
            r#"
            let calc = (x) => {
                let y = x * 2;
                return y + 1;
            };
            return calc(5);
        "#,
        );
        assert!(result.to_number() == 11.0);
    }

    #[test]
    fn arrow_function_no_params() {
        let result = run("let greet = () => 42; return greet();");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn arrow_in_map() {
        let result = run(
            r#"
            let arr = [1, 2, 3];
            let doubled = arr.map(x => x * 2);
            return doubled[0] + doubled[1] + doubled[2];
        "#,
        );
        assert!(result.to_number() == 12.0);
    }

    #[test]
    fn template_literal_basic() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let name = "world"; return `hello ${name}`;"#)
            .unwrap();
        assert_eq!(engine.vm().value_to_string(result), "hello world");
    }

    #[test]
    fn template_literal_expression() {
        let mut engine = Engine::new();
        let result = engine
            .eval(r#"let x = 5; return `result: ${x * 2}`;"#)
            .unwrap();
        assert_eq!(engine.vm().value_to_string(result), "result: 10");
    }

    #[test]
    fn template_literal_no_interpolation() {
        let mut engine = Engine::new();
        let result = engine.eval(r#"return `hello world`;"#).unwrap();
        assert_eq!(engine.vm().value_to_string(result), "hello world");
    }

    #[test]
    fn optional_chaining_defined() {
        let result = run("let obj = {a: {b: 42}}; return obj?.a?.b;");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn optional_chaining_null() {
        let result = run("let obj = null; return obj?.a;");
        assert!(result.is_undefined());
    }

    #[test]
    fn optional_chaining_deep() {
        let result = run("let obj = {a: null}; return obj?.a?.b;");
        assert!(result.is_undefined());
    }

    #[test]
    fn nullish_coalescing_null() {
        let result = run("let x = null; return x ?? 42;");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn nullish_coalescing_undefined() {
        let result = run("let x; return x ?? 42;");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn nullish_coalescing_defined() {
        let result = run("let x = 10; return x ?? 42;");
        assert!(result.to_number() == 10.0);
    }

    #[test]
    fn nullish_coalescing_zero() {
        let result = run("let x = 0; return x ?? 42;");
        assert!(result.to_number() == 0.0);
    }

    #[test]
    fn nullish_coalescing_empty_string() {
        let result = run(r#"let x = ""; return x ?? "default";"#);
        assert!(result.is_string());
    }

    #[test]
    fn set_timeout_basic() {
        let mut engine = Engine::new();
        engine
            .eval("result = 0; setTimeout(function() { result = 42; }, 0);")
            .unwrap();
        engine.run_event_loop().unwrap();
        let result = engine.vm().get_global("result");
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn set_timeout_delay() {
        let mut engine = Engine::new();
        engine
            .eval("result = 0; setTimeout(function() { result = 1; }, 10);")
            .unwrap();
        engine.run_event_loop().unwrap();
        let result = engine.vm().get_global("result");
        assert!(result.to_number() == 1.0);
    }

    #[test]
    fn clear_timeout() {
        let mut engine = Engine::new();
        engine
            .eval(
                r#"
                result = 0;
                let id = setTimeout(function() { result = 99; }, 0);
                clearTimeout(id);
            "#,
            )
            .unwrap();
        engine.run_event_loop().unwrap();
        let result = engine.vm().get_global("result");
        assert!(result.to_number() == 0.0);
    }

    #[test]
    fn set_interval_basic() {
        let mut engine = Engine::new();
        engine
            .eval(
                r#"
                count = 0;
                let id = setInterval(function() {
                    count = count + 1;
                    if (count >= 3) { clearInterval(id); }
                }, 5);
            "#,
            )
            .unwrap();
        engine.run_event_loop().unwrap();
        let result = engine.vm().get_global("count");
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn queue_microtask_basic() {
        let mut engine = Engine::new();
        engine
            .eval(
                r#"
                result = [];
                result.push(1);
                queueMicrotask(function() { result.push(3); });
                result.push(2);
            "#,
            )
            .unwrap();
        let result = engine.vm().get_global("result");
        if let Some(obj) = engine.vm().get_object(result) {
            if let one_vm::object::ObjectKind::Array { length } = obj.kind() {
                assert_eq!(*length, 3);
            }
        }
    }

    #[test]
    fn timeout_ordering() {
        let mut engine = Engine::new();
        engine
            .eval(
                r#"
                order = "";
                setTimeout(function() { order = order + "b"; }, 10);
                setTimeout(function() { order = order + "a"; }, 0);
            "#,
            )
            .unwrap();
        engine.run_event_loop().unwrap();
        let result = engine.vm().get_global("order");
        assert!(result.is_string());
        assert_eq!(engine.vm().value_to_string(result), "ab");
    }
}
