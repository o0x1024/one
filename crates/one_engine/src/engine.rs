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
}
