use one_core::JsValue;
use one_engine::extension::{host_fn, Extension, HostFnDescriptor};

pub struct AstExtension;

impl AstExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for AstExtension {
    fn name(&self) -> &str {
        "sentinel_ast"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![host_fn("__ast_parse_js", |vm, args| {
            let source = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();

            match one_parser::parser::Parser::parse(&source) {
                Ok(_program) => {
                    let source_val = vm.alloc_string(source);
                    let result = vm.create_object_from_pairs(&[
                        ("success".to_string(), JsValue::from_bool(true)),
                        ("source".to_string(), source_val),
                    ]);
                    Ok(result)
                }
                Err(e) => {
                    let err_val = vm.alloc_string(e.message);
                    let result = vm.create_object_from_pairs(&[
                        ("success".to_string(), JsValue::from_bool(false)),
                        ("error".to_string(), err_val),
                    ]);
                    Ok(result)
                }
            }
        })]
    }
}
