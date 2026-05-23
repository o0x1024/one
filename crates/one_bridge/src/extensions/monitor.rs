use one_core::JsValue;
use one_engine::extension::{host_fn, Extension, HostFnDescriptor};

pub struct MonitorExtension;

impl MonitorExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for MonitorExtension {
    fn name(&self) -> &str {
        "sentinel_monitor"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![
            host_fn("__monitor_report_progress", |_vm, _args| {
                Ok(JsValue::undefined())
            }),
            host_fn("__monitor_emit_active_probe_event", |_vm, _args| {
                Ok(JsValue::undefined())
            }),
            host_fn("__monitor_get_settings", |vm, _args| {
                let settings = vm.create_object_from_pairs(&[
                    ("maxConcurrentFetches".to_string(), JsValue::from_i32(10)),
                    ("fetchTimeoutMs".to_string(), JsValue::from_i32(30000)),
                    ("maxActiveProbes".to_string(), JsValue::from_i32(5)),
                ]);
                Ok(settings)
            }),
        ]
    }
}
