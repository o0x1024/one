use one_core::{JsValue, OneResult};
use one_engine::extension::{host_fn, Extension, HostFnDescriptor};
use one_vm::Vm;

pub struct FetchExtension;

impl FetchExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for FetchExtension {
    fn name(&self) -> &str {
        "sentinel_fetch"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![
            host_fn("__sentinel_fetch", sentinel_fetch),
            host_fn("__sentinel_abort_fetch", sentinel_abort_fetch),
        ]
    }
}

fn sentinel_fetch(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let url = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();

    match ureq::get(&url).call() {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = response.into_body().read_to_string().unwrap_or_default();

            let ok_val = JsValue::from_bool((200..300).contains(&status));
            let status_val = JsValue::from_i32(status as i32);
            let body_val = vm.alloc_string(body);
            let result = vm.create_object_from_pairs(&[
                ("ok".to_string(), ok_val),
                ("status".to_string(), status_val),
                ("body".to_string(), body_val),
            ]);
            Ok(result)
        }
        Err(e) => {
            let err_msg = vm.alloc_string(e.to_string());
            let result = vm.create_object_from_pairs(&[
                ("ok".to_string(), JsValue::from_bool(false)),
                ("status".to_string(), JsValue::from_i32(0)),
                ("body".to_string(), err_msg),
            ]);
            Ok(result)
        }
    }
}

fn sentinel_abort_fetch(_vm: &mut Vm, _args: &[JsValue]) -> OneResult<JsValue> {
    Ok(JsValue::undefined())
}
