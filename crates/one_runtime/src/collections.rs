use one_core::OneResult;
use one_core::JsValue;
use one_vm::Vm;
use one_vm::object::{JsObject, MapData, ObjectKind, SetData};

type CollectionMethod = fn(&mut Vm, &[JsValue]) -> OneResult<JsValue>;

fn js_equal(vm: &Vm, a: JsValue, b: JsValue) -> bool {
    if a == b {
        return true;
    }
    if a.is_string() && b.is_string() {
        return vm.value_to_string(a) == vm.value_to_string(b);
    }
    false
}

fn find_map_entry_index(vm: &Vm, entries: &[(JsValue, JsValue)], key: JsValue) -> Option<usize> {
    entries
        .iter()
        .position(|(k, _)| js_equal(vm, *k, key))
}

fn find_set_value_index(vm: &Vm, values: &[JsValue], value: JsValue) -> Option<usize> {
    values.iter().position(|v| js_equal(vm, *v, value))
}

fn values_to_array(vm: &mut Vm, values: &[JsValue]) -> JsValue {
    let result = vm.new_array(values.len() as u32);
    if let Some(obj) = vm.get_object_mut(result) {
        for (i, val) in values.iter().enumerate() {
            obj.set_property(i.to_string(), *val);
        }
    }
    result
}

fn map_constructor(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    Ok(vm.new_map())
}

fn set_constructor(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    Ok(vm.new_set())
}

fn map_set(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let key = args.first().copied().unwrap_or(JsValue::undefined());
    let value = args.get(1).copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");

    let idx = vm.get_object(this).and_then(|obj| {
        if let ObjectKind::Map(data) = obj.kind() {
            find_map_entry_index(vm, &data.entries, key)
        } else {
            None
        }
    });

    let Some(obj) = vm.get_object_mut(this) else {
        return Ok(JsValue::undefined());
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind_mut() else {
        return Ok(JsValue::undefined());
    };

    if let Some(idx) = idx {
        entries[idx].1 = value;
    } else {
        entries.push((key, value));
    }
    Ok(this)
}

fn map_get(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let key = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(JsValue::undefined());
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind() else {
        return Ok(JsValue::undefined());
    };

    Ok(find_map_entry_index(vm, entries, key)
        .map(|idx| entries[idx].1)
        .unwrap_or(JsValue::undefined()))
}

fn map_has(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let key = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(JsValue::from_bool(false));
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind() else {
        return Ok(JsValue::from_bool(false));
    };

    Ok(JsValue::from_bool(
        find_map_entry_index(vm, entries, key).is_some(),
    ))
}

fn map_delete(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let key = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");

    let idx = vm.get_object(this).and_then(|obj| {
        if let ObjectKind::Map(data) = obj.kind() {
            find_map_entry_index(vm, &data.entries, key)
        } else {
            None
        }
    });

    let Some(obj) = vm.get_object_mut(this) else {
        return Ok(JsValue::from_bool(false));
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind_mut() else {
        return Ok(JsValue::from_bool(false));
    };

    if let Some(idx) = idx {
        entries.remove(idx);
        Ok(JsValue::from_bool(true))
    } else {
        Ok(JsValue::from_bool(false))
    }
}

fn map_clear(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object_mut(this) else {
        return Ok(JsValue::undefined());
    };
    if let ObjectKind::Map(MapData { entries }) = obj.kind_mut() {
        entries.clear();
    }
    Ok(JsValue::undefined())
}

fn map_for_each(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(JsValue::undefined());
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind() else {
        return Ok(JsValue::undefined());
    };

    let pairs: Vec<(JsValue, JsValue)> = entries.to_vec();
    for (key, value) in pairs {
        vm.call_function(callback, &[value, key])?;
    }
    Ok(JsValue::undefined())
}

fn map_keys(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(vm.new_array(0));
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind() else {
        return Ok(vm.new_array(0));
    };

    let keys: Vec<JsValue> = entries.iter().map(|(k, _)| *k).collect();
    Ok(values_to_array(vm, &keys))
}

fn map_values(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(vm.new_array(0));
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind() else {
        return Ok(vm.new_array(0));
    };

    let values: Vec<JsValue> = entries.iter().map(|(_, v)| *v).collect();
    Ok(values_to_array(vm, &values))
}

fn map_entries(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(vm.new_array(0));
    };
    let ObjectKind::Map(MapData { entries }) = obj.kind() else {
        return Ok(vm.new_array(0));
    };

    let pairs: Vec<(JsValue, JsValue)> = entries.to_vec();
    let mut pair_values = Vec::with_capacity(pairs.len());
    for (key, value) in pairs {
        let pair = vm.new_array(2);
        if let Some(pair_obj) = vm.get_object_mut(pair) {
            pair_obj.set_property("0".to_string(), key);
            pair_obj.set_property("1".to_string(), value);
        }
        pair_values.push(pair);
    }

    let result = vm.new_array(pair_values.len() as u32);
    if let Some(result_obj) = vm.get_object_mut(result) {
        for (i, pair) in pair_values.iter().enumerate() {
            result_obj.set_property(i.to_string(), *pair);
        }
    }
    Ok(result)
}

fn set_add(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let value = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");

    let exists = vm.get_object(this).is_some_and(|obj| {
        matches!(obj.kind(), ObjectKind::Set(SetData { values }) if find_set_value_index(vm, values, value).is_some())
    });

    if exists {
        return Ok(this);
    }

    let Some(obj) = vm.get_object_mut(this) else {
        return Ok(JsValue::undefined());
    };
    let ObjectKind::Set(SetData { values }) = obj.kind_mut() else {
        return Ok(JsValue::undefined());
    };

    values.push(value);
    Ok(this)
}

fn set_has(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let value = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(JsValue::from_bool(false));
    };
    let ObjectKind::Set(SetData { values }) = obj.kind() else {
        return Ok(JsValue::from_bool(false));
    };

    Ok(JsValue::from_bool(
        find_set_value_index(vm, values, value).is_some(),
    ))
}

fn set_delete(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let value = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");

    let idx = vm.get_object(this).and_then(|obj| {
        if let ObjectKind::Set(SetData { values }) = obj.kind() {
            find_set_value_index(vm, values, value)
        } else {
            None
        }
    });

    let Some(obj) = vm.get_object_mut(this) else {
        return Ok(JsValue::from_bool(false));
    };
    let ObjectKind::Set(SetData { values }) = obj.kind_mut() else {
        return Ok(JsValue::from_bool(false));
    };

    if let Some(idx) = idx {
        values.remove(idx);
        Ok(JsValue::from_bool(true))
    } else {
        Ok(JsValue::from_bool(false))
    }
}

fn set_clear(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object_mut(this) else {
        return Ok(JsValue::undefined());
    };
    if let ObjectKind::Set(SetData { values }) = obj.kind_mut() {
        values.clear();
    }
    Ok(JsValue::undefined())
}

fn set_for_each(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(JsValue::undefined());
    };
    let ObjectKind::Set(SetData { values }) = obj.kind() else {
        return Ok(JsValue::undefined());
    };

    let items: Vec<JsValue> = values.to_vec();
    for value in items {
        vm.call_function(callback, &[value])?;
    }
    Ok(JsValue::undefined())
}

fn install_proto_method(
    vm: &mut Vm,
    type_name: &str,
    proto_val: JsValue,
    name: &str,
    func: CollectionMethod,
) {
    let host_name = format!("{type_name}.prototype.{name}");
    let sentinel = vm.register_host_fn_returning_sentinel(&host_name, func);
    if let Some(obj) = vm.get_object_mut(proto_val) {
        obj.set_property(name.to_string(), sentinel);
    }
}

fn install_collection(
    vm: &mut Vm,
    type_name: &str,
    constructor: CollectionMethod,
    methods: &[(&str, CollectionMethod)],
    set_prototype: fn(&mut Vm, JsValue),
) {
    vm.register_host_fn(type_name, constructor);

    let proto = JsObject::new();
    let proto_val = vm.alloc_object(proto);
    for (name, func) in methods {
        install_proto_method(vm, type_name, proto_val, name, *func);
    }
    set_prototype(vm, proto_val);

    let global = vm.get_global(type_name);
    if let Some(obj) = vm.get_object_mut(global) {
        obj.set_property("prototype".to_string(), proto_val);
    }
}

pub fn install_collections(vm: &mut Vm) {
    install_collection(
        vm,
        "Map",
        map_constructor,
        &[
            ("set", map_set),
            ("get", map_get),
            ("has", map_has),
            ("delete", map_delete),
            ("clear", map_clear),
            ("forEach", map_for_each),
            ("keys", map_keys),
            ("values", map_values),
            ("entries", map_entries),
        ],
        Vm::set_map_prototype,
    );

    install_collection(
        vm,
        "Set",
        set_constructor,
        &[
            ("add", set_add),
            ("has", set_has),
            ("delete", set_delete),
            ("clear", set_clear),
            ("forEach", set_for_each),
        ],
        Vm::set_set_prototype,
    );
}
