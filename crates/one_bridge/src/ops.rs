use one_core::{JsValue, OneResult};
use one_engine::Engine;
use one_vm::object::ObjectKind;
use one_vm::Vm;
use tracing::{debug, error, info, warn};

pub fn install_plugin_ops<T: 'static>(engine: &mut Engine<T>) {
    let vm = engine.vm_mut();

    vm.register_host_fn("Sentinel.log", sentinel_log);
    vm.register_host_fn("Sentinel.emitFinding", sentinel_emit_finding);
    vm.register_host_fn("Sentinel.return", sentinel_return);
}

fn push_to_array(vm: &mut Vm, global_name: &str, value: JsValue) {
    let arr_val = vm.get_global(global_name);
    let Some(obj) = vm.get_object_mut(arr_val) else {
        return;
    };
    let idx = match obj.kind() {
        ObjectKind::Array { length } => *length,
        _ => return,
    };
    obj.set_property(idx.to_string(), value);
    if let ObjectKind::Array { length } = obj.kind_mut() {
        *length = idx + 1;
    }
}

fn sentinel_log(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let level = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let message = args
        .get(1)
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();

    match level.to_ascii_lowercase().as_str() {
        "error" => error!("[Plugin] {message}"),
        "warn" => warn!("[Plugin] {message}"),
        "info" => info!("[Plugin] {message}"),
        _ => debug!("[Plugin] {message}"),
    }

    let level_val = vm.alloc_string(level);
    let message_val = vm.alloc_string(message);
    let entry = vm.create_object_from_pairs(&[
        ("level".to_string(), level_val),
        ("message".to_string(), message_val),
    ]);
    push_to_array(vm, "__sentinel_logs__", entry);

    Ok(JsValue::undefined())
}

fn sentinel_emit_finding(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let finding = args.first().copied().unwrap_or(JsValue::undefined());
    push_to_array(vm, "__sentinel_findings__", finding);
    Ok(JsValue::undefined())
}

fn sentinel_return(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let value = args.first().copied().unwrap_or(JsValue::undefined());
    vm.set_global("__sentinel_last_result__", value);
    Ok(JsValue::undefined())
}
