use one_core::JsValue;
use one_vm::{PromiseState, Vm};

/// Install Promise constructor and static methods on the VM
pub fn install_promise(vm: &mut Vm) {
    vm.register_host_fn("Promise", |vm, args| {
        let executor = args.first().copied().unwrap_or(JsValue::undefined());
        let promise_val = vm.alloc_promise(PromiseState::Pending {
            on_fulfilled: Vec::new(),
            on_rejected: Vec::new(),
        });

        let resolve = vm.create_promise_resolver(promise_val, true);
        let reject = vm.create_promise_resolver(promise_val, false);

        vm.call_function(executor, &[resolve, reject])?;
        Ok(promise_val)
    });

    vm.register_host_fn("Promise.resolve", |vm, args| {
        let value = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_promise(PromiseState::Fulfilled(value)))
    });

    vm.register_host_fn("Promise.reject", |vm, args| {
        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_promise(PromiseState::Rejected(reason)))
    });
}
