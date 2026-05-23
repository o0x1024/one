use one_core::{JsValue, OneResult};
use one_vm::Vm;

use crate::type_map::TypeMap;

pub struct HostFnDescriptor {
    pub name: String,
    pub func: Box<dyn Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue> + 'static>,
}

pub trait Extension: Send + 'static {
    fn name(&self) -> &str;
    fn host_functions(&self) -> Vec<HostFnDescriptor>;
    fn globals(&self) -> Vec<(String, JsValue)> {
        vec![]
    }
    fn bootstrap_js(&self) -> Option<&str> {
        None
    }
    fn init_state(&self, _state: &mut TypeMap) {}
}

pub fn host_fn<F>(name: &str, func: F) -> HostFnDescriptor
where
    F: Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue> + 'static,
{
    HostFnDescriptor {
        name: name.to_string(),
        func: Box::new(func),
    }
}
