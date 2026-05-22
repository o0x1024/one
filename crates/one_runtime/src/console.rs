use one_core::JsValue;
use one_vm::Vm;

/// Install console.* methods as host functions on the VM
pub fn install_console(vm: &mut Vm) {
    vm.register_host_fn("console.log", |vm, args| {
        let parts: Vec<String> = args.iter().map(|a| vm.value_to_string(*a)).collect();
        println!("{}", parts.join(" "));
        Ok(JsValue::undefined())
    });

    vm.register_host_fn("console.warn", |vm, args| {
        let parts: Vec<String> = args.iter().map(|a| vm.value_to_string(*a)).collect();
        eprintln!("{}", parts.join(" "));
        Ok(JsValue::undefined())
    });

    vm.register_host_fn("console.error", |vm, args| {
        let parts: Vec<String> = args.iter().map(|a| vm.value_to_string(*a)).collect();
        eprintln!("{}", parts.join(" "));
        Ok(JsValue::undefined())
    });
}
