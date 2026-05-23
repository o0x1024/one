use one_core::{JsValue, OneResult};
use one_engine::extension::{host_fn, Extension, HostFnDescriptor};
use one_vm::Vm;

pub struct SentinelCoreExtension;

impl SentinelCoreExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for SentinelCoreExtension {
    fn name(&self) -> &str {
        "sentinel_core"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![
            host_fn("__sentinel_log", sentinel_log),
            host_fn("__sentinel_emit_finding", sentinel_emit_finding),
            host_fn("__sentinel_return", sentinel_return),
            host_fn("sleep", |_vm, args| {
                let ms = args
                    .first()
                    .map(|v| v.to_number())
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .unwrap_or(0.0) as u64;
                std::thread::sleep(std::time::Duration::from_millis(ms));
                Ok(JsValue::undefined())
            }),
        ]
    }

    fn bootstrap_js(&self) -> Option<&str> {
        Some(include_str!("../bootstrap.js"))
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

    let logs_arr = vm.get_global("__sentinel_logs__");
    let level_val = vm.alloc_string(level.clone());
    let message_val = vm.alloc_string(message.clone());
    let entry = vm.create_object_from_pairs(&[
        ("level".to_string(), level_val),
        ("message".to_string(), message_val),
    ]);

    if let Some(obj) = vm.get_object_mut(logs_arr) {
        if let one_vm::ObjectKind::Array { length } = obj.kind_mut() {
            let idx = *length;
            *length += 1;
            obj.set_property(idx.to_string(), entry);
        }
    }

    match level.as_str() {
        "error" => tracing::error!("{message}"),
        "warn" => tracing::warn!("{message}"),
        "debug" => tracing::debug!("{message}"),
        "trace" => tracing::trace!("{message}"),
        _ => tracing::info!("{message}"),
    }

    Ok(JsValue::undefined())
}

fn sentinel_emit_finding(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let finding_val = args.first().copied().unwrap_or(JsValue::undefined());
    let findings_arr = vm.get_global("__sentinel_findings__");

    if let Some(obj) = vm.get_object_mut(findings_arr) {
        if let one_vm::ObjectKind::Array { length } = obj.kind_mut() {
            let idx = *length;
            *length += 1;
            obj.set_property(idx.to_string(), finding_val);
        }
    }

    Ok(JsValue::undefined())
}

fn sentinel_return(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let val = args.first().copied().unwrap_or(JsValue::undefined());
    vm.set_global("__sentinel_last_result__", val);
    Ok(JsValue::undefined())
}
