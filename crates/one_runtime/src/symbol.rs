use one_core::JsValue;
use one_vm::Vm;

pub fn install_symbol(vm: &mut Vm) {
    vm.register_host_fn("Symbol", |vm, args| {
        let desc = args.first().map(|v| vm.value_to_string(*v));
        let id = vm.create_symbol(desc);
        Ok(JsValue::from_symbol_raw(id))
    });

    vm.register_host_fn("Symbol.for", |vm, args| {
        let key = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        let id = vm.get_or_create_global_symbol(&key);
        Ok(JsValue::from_symbol_raw(id))
    });

    // Well-known symbol: Symbol.iterator
    let iter_id = vm.get_or_create_global_symbol("Symbol.iterator");
    let iter_sym = JsValue::from_symbol_raw(iter_id);
    let sym_val = vm.get_global("Symbol");
    if let Some(obj) = vm.get_object_mut(sym_val) {
        obj.set_property("iterator".to_string(), iter_sym);
    }
}
