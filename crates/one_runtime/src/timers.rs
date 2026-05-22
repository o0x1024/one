use one_core::JsValue;
use one_vm::Vm;

pub fn install_timers(vm: &mut Vm) {
    vm.register_host_fn("setTimeout", |vm, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        let delay = args.get(1).map(|v| v.to_number() as u64).unwrap_or(0);
        let id = vm.schedule_timer(callback, delay, false);
        Ok(JsValue::from_i32(id as i32))
    });

    vm.register_host_fn("clearTimeout", |vm, args| {
        let id = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
        vm.cancel_timer(id);
        Ok(JsValue::undefined())
    });

    vm.register_host_fn("setInterval", |vm, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        let delay = args.get(1).map(|v| v.to_number() as u64).unwrap_or(0);
        let id = vm.schedule_timer(callback, delay, true);
        Ok(JsValue::from_i32(id as i32))
    });

    vm.register_host_fn("clearInterval", |vm, args| {
        let id = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
        vm.cancel_timer(id);
        Ok(JsValue::undefined())
    });

    vm.register_host_fn("queueMicrotask", |vm, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        vm.schedule_microtask(callback, JsValue::undefined());
        Ok(JsValue::undefined())
    });
}
