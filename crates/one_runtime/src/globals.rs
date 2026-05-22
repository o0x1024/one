use one_compiler::Compiler;
use one_core::{CompileError, JsValue, OneError};
use one_parser::parser::Parser;
use one_vm::Vm;

pub fn install_globals(vm: &mut Vm) {
    vm.register_host_fn("eval", |vm, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if !arg.is_string() {
            return Ok(arg);
        }

        let mut code_str = vm.value_to_string(arg);
        let trimmed = code_str.trim_end();
        if !trimmed.is_empty() && !trimmed.ends_with(';') && !trimmed.ends_with('}') {
            code_str = format!("{trimmed};");
        }

        let program = Parser::parse(&code_str).map_err(|e| {
            OneError::CompileError(CompileError {
                message: e.message,
                file: Some("<eval>".into()),
                line: 0,
                column: 0,
            })
        })?;

        let code = Compiler::compile_eval(&program);
        vm.execute_inner(&code)
    });
}
