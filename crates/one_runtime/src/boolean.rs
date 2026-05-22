use one_vm::Vm;

pub fn install_boolean(vm: &mut Vm) {
    vm.register_host_fn("Boolean.prototype.toString", |vm, _args| {
        let this = vm.get_global("this");
        let s = if this.as_bool().unwrap_or(false) {
            "true"
        } else {
            "false"
        };
        Ok(vm.alloc_string(s.to_string()))
    });
}
