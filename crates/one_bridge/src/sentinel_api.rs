use one_core::JsValue;
use one_engine::Engine;
use one_vm::object::{JsObject, ObjectKind};
use one_vm::Vm;

pub fn init_storage(vm: &mut Vm) {
    let findings_arr = create_empty_array(vm);
    vm.set_global("__sentinel_findings__", findings_arr);

    let logs_arr = create_empty_array(vm);
    vm.set_global("__sentinel_logs__", logs_arr);

    vm.set_global("__sentinel_last_result__", JsValue::undefined());
}

pub fn install_sentinel_api<T: 'static>(engine: &mut Engine<T>) {
    let vm = engine.vm_mut();
    init_storage(vm);

    vm.register_host_fn("sleep", |_vm, args| {
        let ms = args
            .first()
            .map(|v| v.to_number())
            .filter(|n| n.is_finite() && *n >= 0.0)
            .unwrap_or(0.0) as u64;
        std::thread::sleep(std::time::Duration::from_millis(ms));
        Ok(JsValue::undefined())
    });
}

fn create_empty_array(vm: &mut Vm) -> JsValue {
    vm.alloc_object(JsObject::with_kind(ObjectKind::Array { length: 0 }))
}
