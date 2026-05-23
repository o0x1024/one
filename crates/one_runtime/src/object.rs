use one_core::JsValue;
use one_vm::Vm;
use one_vm::object::{JsObject, ObjectKind, Property};

fn empty_array(vm: &mut Vm) -> JsValue {
    vm.alloc_object(JsObject::with_kind(ObjectKind::Array { length: 0 }))
}

fn alloc_array(vm: &mut Vm, items: Vec<JsValue>) -> JsValue {
    let len = items.len() as u32;
    let arr_val = vm.alloc_object(JsObject::with_kind(ObjectKind::Array { length: len }));
    if let Some(arr_obj) = vm.get_object_mut(arr_val) {
        for (i, item) in items.into_iter().enumerate() {
            arr_obj.set_property(i.to_string(), item);
        }
    }
    arr_val
}

fn descriptor_bool(obj: &JsObject, key: &str, default: bool) -> bool {
    match obj.get_property(key) {
        Some(val) if val.is_boolean() => val.as_bool().unwrap(),
        _ => default,
    }
}

pub fn install_object(vm: &mut Vm) {
    vm.register_host_fn("Object.keys", |vm, args| {
        let obj_val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(obj) = vm.get_object(obj_val) {
            let keys = obj.enumerable_keys();
            let items: Vec<JsValue> = keys
                .into_iter()
                .map(|key| vm.alloc_string(key))
                .collect();
            Ok(alloc_array(vm, items))
        } else {
            Ok(empty_array(vm))
        }
    });

    vm.register_host_fn("Object.values", |vm, args| {
        let obj_val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(obj) = vm.get_object(obj_val) {
            Ok(alloc_array(vm, obj.own_property_values()))
        } else {
            Ok(empty_array(vm))
        }
    });

    vm.register_host_fn("Object.entries", |vm, args| {
        let obj_val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(obj) = vm.get_object(obj_val) {
            let entries: Vec<JsValue> = obj
                .own_entries()
                .into_iter()
                .map(|(key, value)| {
                    let key_val = vm.alloc_string(key);
                    alloc_array(vm, vec![key_val, value])
                })
                .collect();
            Ok(alloc_array(vm, entries))
        } else {
            Ok(empty_array(vm))
        }
    });

    vm.register_host_fn("Object.assign", |vm, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        if vm.get_object_mut(target).is_none() {
            return Ok(JsValue::undefined());
        }

        for &source in args.iter().skip(1) {
            if let Some(src_obj) = vm.get_object(source) {
                for (key, value) in src_obj.own_entries() {
                    if let Some(target_obj) = vm.get_object_mut(target) {
                        target_obj.set_property(key, value);
                    }
                }
            }
        }
        Ok(target)
    });

    vm.register_host_fn("Object.freeze", |vm, args| {
        let obj_val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(obj) = vm.get_object_mut(obj_val) {
            obj.freeze();
        }
        Ok(obj_val)
    });

    vm.register_host_fn("Object.create", |vm, args| {
        let proto_val = args.first().copied().unwrap_or(JsValue::undefined());
        let mut obj = JsObject::new();
        if !proto_val.is_null()
            && !proto_val.is_undefined()
            && let Some(raw) = proto_val.as_object_raw()
        {
            obj.set_prototype(Some(raw as *mut JsObject));
        }
        Ok(vm.alloc_object(obj))
    });

    vm.register_host_fn("Object.getPrototypeOf", |vm, args| {
        let obj_val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(obj) = vm.get_object(obj_val) {
            if let Some(proto) = obj.prototype() {
                Ok(JsValue::from_object_raw(proto as u64))
            } else {
                Ok(JsValue::null())
            }
        } else {
            Ok(JsValue::undefined())
        }
    });

    vm.register_host_fn("Object.hasOwn", |vm, args| {
        let obj_val = args.first().copied().unwrap_or(JsValue::undefined());
        let key_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let key = vm.value_to_string(key_val);
        if let Some(obj) = vm.get_object(obj_val) {
            Ok(JsValue::from_bool(obj.has_own_property(&key)))
        } else {
            Ok(JsValue::from_bool(false))
        }
    });

    vm.register_host_fn("Object.defineProperty", |vm, args| {
        let obj_val = args.first().copied().unwrap_or(JsValue::undefined());
        let key_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let desc_val = args.get(2).copied().unwrap_or(JsValue::undefined());
        let key = vm.value_to_string(key_val);

        let prop = vm.get_object(desc_val).map(|desc| {
            let value = desc.get_property("value").unwrap_or(JsValue::undefined());
            Property {
                value,
                writable: descriptor_bool(desc, "writable", false),
                enumerable: descriptor_bool(desc, "enumerable", false),
                configurable: descriptor_bool(desc, "configurable", false),
            }
        });

        if let (Some(obj), Some(prop)) = (vm.get_object_mut(obj_val), prop) {
            obj.define_property(key, prop);
        }
        Ok(obj_val)
    });
}
