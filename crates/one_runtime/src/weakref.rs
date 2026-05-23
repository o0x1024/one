use one_core::JsValue;
use one_vm::object::{JsObject, ObjectKind};
use one_vm::Vm;

fn weakref_target_alive(vm: &Vm, target: JsValue) -> bool {
    if target.is_object()
        && let Some(raw) = target.as_object_raw()
    {
        return vm.heap_contains_ptr(raw as *const u8);
    }
    false
}

fn weakref_deref(vm: &Vm, weak_ref_val: JsValue) -> JsValue {
    vm.get_object(weak_ref_val)
        .and_then(|obj| {
            if let ObjectKind::WeakRef(target) = obj.kind() {
                if weakref_target_alive(vm, *target) {
                    Some(*target)
                } else {
                    Some(JsValue::undefined())
                }
            } else {
                None
            }
        })
        .unwrap_or(JsValue::undefined())
}

fn weakref_constructor(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let target = args.first().copied().unwrap_or(JsValue::undefined());
    Ok(vm.new_weakref(target))
}

fn weakref_deref_method(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    Ok(weakref_deref(vm, this))
}

fn install_proto_method<F>(vm: &mut Vm, proto_val: JsValue, name: &str, func: F)
where
    F: Fn(&mut Vm, &[JsValue]) -> one_core::OneResult<JsValue> + 'static,
{
    let host_name = format!("WeakRef.prototype.{name}");
    let sentinel = vm.register_host_fn_returning_sentinel(&host_name, func);
    if let Some(obj) = vm.get_object_mut(proto_val) {
        obj.set_property(name.to_string(), sentinel);
    }
}

pub fn install_weakref(vm: &mut Vm) {
    vm.register_host_fn("WeakRef", weakref_constructor);

    let proto = JsObject::new();
    let proto_val = vm.alloc_object(proto);

    install_proto_method(vm, proto_val, "deref", weakref_deref_method);

    vm.set_weakref_prototype(proto_val);

    let weakref_global = vm.get_global("WeakRef");
    if let Some(obj) = vm.get_object_mut(weakref_global) {
        obj.set_property("prototype".to_string(), proto_val);
    }
}
