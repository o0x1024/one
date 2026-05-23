use one_engine::extension::{host_fn, Extension, HostFnDescriptor};

pub struct NetworkExtension;

impl NetworkExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for NetworkExtension {
    fn name(&self) -> &str {
        "sentinel_network"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![
            host_fn("__network_scan_ports", |vm, _args| {
                Ok(vm.new_array(0))
            }),
            host_fn("__network_probe_services", |vm, _args| {
                Ok(vm.new_array(0))
            }),
            host_fn("__network_get_capabilities", |vm, _args| {
                let engines = vm.new_array(0);
                let result = vm.create_object_from_pairs(&[(
                    "engines".to_string(),
                    engines,
                )]);
                Ok(result)
            }),
        ]
    }
}
