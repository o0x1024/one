use one_vm::Vm;
use one_vm::object::JsObject;

fn register_error_type(vm: &mut Vm, name: &str) {
    let error_name = name.to_string();
    vm.register_host_fn(name, move |vm, args| {
        let message = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        let mut obj = JsObject::new();
        obj.set_property("name".to_string(), vm.alloc_string(error_name.clone()));
        obj.set_property("message".to_string(), vm.alloc_string(message));
        Ok(vm.alloc_object(obj))
    });
}

pub fn install_error(vm: &mut Vm) {
    for name in [
        "Error",
        "TypeError",
        "ReferenceError",
        "SyntaxError",
        "RangeError",
        "URIError",
        "EvalError",
    ] {
        register_error_type(vm, name);
    }
}
