use std::net::ToSocketAddrs;

use one_core::{JsValue, OneResult};
use one_vm::Vm;

pub fn install_dns(vm: &mut Vm) {
    vm.register_host_fn("dns.resolve", dns_resolve);
    vm.register_host_fn("dns.lookup", dns_lookup);
}

fn dns_resolve(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let host = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();

    let lookup = format!("{host}:0");
    match lookup.to_socket_addrs() {
        Ok(addrs) => {
            let addresses: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
            let arr = vm.new_array(0);
            for addr in addresses {
                let val = vm.alloc_string(addr);
                push_to_array(vm, arr, val);
            }
            Ok(arr)
        }
        Err(e) => {
            let err = vm.alloc_string(e.to_string());
            Ok(vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]))
        }
    }
}

fn dns_lookup(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let host = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();

    let lookup = format!("{host}:0");
    match lookup.to_socket_addrs() {
        Ok(mut addrs) => {
            if let Some(addr) = addrs.next() {
                Ok(vm.alloc_string(addr.ip().to_string()))
            } else {
                Ok(JsValue::null())
            }
        }
        Err(_) => Ok(JsValue::null()),
    }
}

fn push_to_array(vm: &mut Vm, arr: JsValue, val: JsValue) {
    if let Some(func) = vm.get_primitive_method("Array", "push") {
        let _ = vm.call_function(func, &[arr, val]);
    }
}
