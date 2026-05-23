use one_core::JsValue;
use one_engine::extension::{host_fn, Extension, HostFnDescriptor};

pub struct TlsExtension;

impl TlsExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for TlsExtension {
    fn name(&self) -> &str {
        "sentinel_tls"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![
            host_fn("__tls_peer_certificate", |_vm, _args| {
                Ok(JsValue::null())
            }),
            host_fn("__tls_get_certificate", |vm, args| {
                let _host = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
                let _port = args.get(1).map(|v| v.to_number() as u16).unwrap_or(443);
                Ok(JsValue::null())
            }),
        ]
    }
}
