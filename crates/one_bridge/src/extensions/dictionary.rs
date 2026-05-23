use one_core::JsValue;
use one_engine::extension::{host_fn, Extension, HostFnDescriptor};

pub struct DictionaryExtension;

impl DictionaryExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for DictionaryExtension {
    fn name(&self) -> &str {
        "sentinel_dictionary"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![
            host_fn("__dict_get_dictionary", |_vm, _args| {
                Ok(JsValue::null())
            }),
            host_fn("__dict_get_default_id", |_vm, _args| {
                Ok(JsValue::null())
            }),
            host_fn("__dict_get_words", |vm, _args| {
                Ok(vm.new_array(0))
            }),
            host_fn("__dict_get_entries", |vm, _args| {
                Ok(vm.new_array(0))
            }),
            host_fn("__dict_list", |vm, _args| {
                Ok(vm.new_array(0))
            }),
        ]
    }
}
