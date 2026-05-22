use one_core::JsValue;
use one_vm::Vm;
use one_vm::object::{JsObject, ObjectKind};

fn get_array_element(obj: &JsObject, index: u32) -> JsValue {
    obj.get_property(&index.to_string())
        .unwrap_or(JsValue::undefined())
}

fn collect_elements(vm: &Vm, arr_val: JsValue) -> Vec<JsValue> {
    let Some(obj) = vm.get_object(arr_val) else {
        return Vec::new();
    };
    let ObjectKind::Array { length } = obj.kind() else {
        return Vec::new();
    };
    (0..*length)
        .map(|i| get_array_element(obj, i))
        .collect()
}

fn normalize_index(index: i32, len: i32) -> i32 {
    let mut idx = index;
    if idx < 0 {
        idx += len;
    }
    idx.clamp(0, len)
}

fn set_array_elements(vm: &mut Vm, arr_val: JsValue, elements: &[JsValue]) {
    let Some(obj) = vm.get_object_mut(arr_val) else {
        return;
    };
    let old_len = match obj.kind() {
        ObjectKind::Array { length } => *length,
        _ => return,
    };

    for i in 0..old_len {
        obj.delete_property(&i.to_string());
    }
    for (i, val) in elements.iter().enumerate() {
        obj.set_property(i.to_string(), *val);
    }
    if let ObjectKind::Array { length } = obj.kind_mut() {
        *length = elements.len() as u32;
    }
}

fn is_truthy(vm: &Vm, val: JsValue) -> bool {
    if val.is_null() || val.is_undefined() {
        return false;
    }
    if val.is_boolean() {
        return val.as_bool().unwrap();
    }
    if val.is_int32() {
        return val.as_i32().unwrap() != 0;
    }
    if val.is_float64() {
        let n = val.as_f64().unwrap();
        return n != 0.0 && !n.is_nan();
    }
    if val.is_string() {
        return !vm.value_to_string(val).is_empty();
    }
    true
}

fn array_push(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object_mut(this) else {
        return Ok(JsValue::undefined());
    };
    let start = match obj.kind() {
        ObjectKind::Array { length } => *length,
        _ => return Ok(JsValue::undefined()),
    };

    for (i, &val) in args.iter().enumerate() {
        obj.set_property((start + i as u32).to_string(), val);
    }
    let new_len = start + args.len() as u32;
    if let ObjectKind::Array { length } = obj.kind_mut() {
        *length = new_len;
    }
    Ok(JsValue::from_i32(new_len as i32))
}

fn array_pop(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let current_len = vm.get_object(this).and_then(|obj| {
        if let ObjectKind::Array { length } = obj.kind() {
            Some(*length)
        } else {
            None
        }
    });
    let Some(mut len) = current_len else {
        return Ok(JsValue::undefined());
    };
    if len == 0 {
        return Ok(JsValue::undefined());
    }
    len -= 1;
    let key = len.to_string();
    let val = vm
        .get_object(this)
        .and_then(|obj| obj.get_property(&key))
        .unwrap_or(JsValue::undefined());
    if let Some(obj) = vm.get_object_mut(this) {
        obj.delete_property(&key);
        if let ObjectKind::Array { length } = obj.kind_mut() {
            *length = len;
        }
    }
    Ok(val)
}

fn array_shift(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);
    if elements.is_empty() {
        return Ok(JsValue::undefined());
    }
    let first = elements[0];
    set_array_elements(vm, this, &elements[1..]);
    Ok(first)
}

fn array_unshift(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);
    let new_len = args.len() + elements.len();
    let mut combined = Vec::with_capacity(new_len);
    combined.extend_from_slice(args);
    combined.extend(elements);
    set_array_elements(vm, this, &combined);
    Ok(JsValue::from_i32(new_len as i32))
}

fn array_index_of(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let search = args.first().copied().unwrap_or(JsValue::undefined());
    let from_index = args.get(1).map(|v| v.to_number() as i32).unwrap_or(0);
    let this = vm.get_global("this");
    let Some(obj) = vm.get_object(this) else {
        return Ok(JsValue::from_i32(-1));
    };
    let ObjectKind::Array { length } = obj.kind() else {
        return Ok(JsValue::from_i32(-1));
    };

    let len = *length as i32;
    let start = normalize_index(from_index, len);
    for i in start..len {
        let val = get_array_element(obj, i as u32);
        if val == search {
            return Ok(JsValue::from_i32(i));
        }
    }
    Ok(JsValue::from_i32(-1))
}

fn array_includes(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let result = array_index_of(vm, args)?;
    Ok(JsValue::from_bool(result.as_i32() != Some(-1)))
}

fn array_slice(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);
    let len = elements.len() as i32;

    let start = args
        .first()
        .map(|v| v.to_number() as i32)
        .unwrap_or(0);
    let end = args
        .get(1)
        .map(|v| v.to_number() as i32)
        .unwrap_or(len);

    let start = normalize_index(start, len);
    let end = if end < 0 {
        (len + end).clamp(0, len)
    } else {
        end.min(len)
    };

    if start >= end {
        return Ok(vm.new_array(0));
    }

    let slice = &elements[start as usize..end as usize];
    let result = vm.new_array(slice.len() as u32);
    if let Some(obj) = vm.get_object_mut(result) {
        for (i, val) in slice.iter().enumerate() {
            obj.set_property(i.to_string(), *val);
        }
    }
    Ok(result)
}

fn array_concat(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let mut combined = collect_elements(vm, this);
    for &arg in args {
        if let Some(obj) = vm.get_object(arg)
            && matches!(obj.kind(), ObjectKind::Array { .. })
        {
            combined.extend(collect_elements(vm, arg));
            continue;
        }
        combined.push(arg);
    }

    let result = vm.new_array(combined.len() as u32);
    if let Some(obj) = vm.get_object_mut(result) {
        for (i, val) in combined.iter().enumerate() {
            obj.set_property(i.to_string(), *val);
        }
    }
    Ok(result)
}

fn array_join(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let separator = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_else(|| ",".to_string());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);
    let joined = elements
        .iter()
        .map(|val| vm.value_to_string(*val))
        .collect::<Vec<_>>()
        .join(&separator);
    Ok(vm.alloc_string(joined))
}

fn array_reverse(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let mut elements = collect_elements(vm, this);
    elements.reverse();
    set_array_elements(vm, this, &elements);
    Ok(this)
}

fn array_map(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);
    let result = vm.new_array(elements.len() as u32);

    for (i, elem) in elements.iter().enumerate() {
        let mapped = vm.call_function(callback, &[*elem, JsValue::from_i32(i as i32)])?;
        if let Some(obj) = vm.get_object_mut(result) {
            obj.set_property(i.to_string(), mapped);
        }
    }
    Ok(result)
}

fn array_filter(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);
    let mut kept = Vec::new();

    for (i, elem) in elements.iter().enumerate() {
        let keep = vm.call_function(callback, &[*elem, JsValue::from_i32(i as i32)])?;
        if is_truthy(vm, keep) {
            kept.push(*elem);
        }
    }

    let result = vm.new_array(kept.len() as u32);
    if let Some(obj) = vm.get_object_mut(result) {
        for (i, val) in kept.iter().enumerate() {
            obj.set_property(i.to_string(), *val);
        }
    }
    Ok(result)
}

fn array_reduce(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);

    if elements.is_empty() {
        return Ok(args.get(1).copied().unwrap_or(JsValue::undefined()));
    }

    let (start_idx, mut acc) = if args.len() > 1 {
        (0, args[1])
    } else {
        (1, elements[0])
    };

    for (i, elem) in elements.iter().enumerate().skip(start_idx) {
        acc = vm.call_function(callback, &[acc, *elem, JsValue::from_i32(i as i32)])?;
    }
    Ok(acc)
}

fn array_for_each(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);

    for (i, elem) in elements.iter().enumerate() {
        vm.call_function(callback, &[*elem, JsValue::from_i32(i as i32)])?;
    }
    Ok(JsValue::undefined())
}

fn array_find(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);

    for (i, elem) in elements.iter().enumerate() {
        let matched = vm.call_function(callback, &[*elem, JsValue::from_i32(i as i32)])?;
        if is_truthy(vm, matched) {
            return Ok(*elem);
        }
    }
    Ok(JsValue::undefined())
}

fn array_some(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);

    for (i, elem) in elements.iter().enumerate() {
        let matched = vm.call_function(callback, &[*elem, JsValue::from_i32(i as i32)])?;
        if is_truthy(vm, matched) {
            return Ok(JsValue::from_bool(true));
        }
    }
    Ok(JsValue::from_bool(false))
}

fn array_every(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let callback = args.first().copied().unwrap_or(JsValue::undefined());
    let this = vm.get_global("this");
    let elements = collect_elements(vm, this);

    for (i, elem) in elements.iter().enumerate() {
        let matched = vm.call_function(callback, &[*elem, JsValue::from_i32(i as i32)])?;
        if !is_truthy(vm, matched) {
            return Ok(JsValue::from_bool(false));
        }
    }
    Ok(JsValue::from_bool(true))
}

fn array_from(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let source = args.first().copied().unwrap_or(JsValue::undefined());

    let elements = if let Some(obj) = vm.get_object(source) {
        if let ObjectKind::Array { length } = obj.kind() {
            (0..*length)
                .map(|i| get_array_element(obj, i))
                .collect::<Vec<_>>()
        } else if let Some(len_val) = obj.get_property("length") {
            let len = len_val.to_number().max(0.0) as u32;
            (0..len)
                .map(|i| {
                    obj.get_property(&i.to_string())
                        .unwrap_or(JsValue::undefined())
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let result = vm.new_array(elements.len() as u32);
    if let Some(result_obj) = vm.get_object_mut(result) {
        for (i, val) in elements.iter().enumerate() {
            result_obj.set_property(i.to_string(), *val);
        }
    }
    Ok(result)
}

fn install_proto_method<F>(vm: &mut Vm, proto_val: JsValue, name: &str, func: F)
where
    F: Fn(&mut Vm, &[JsValue]) -> one_core::OneResult<JsValue> + 'static,
{
    let host_name = format!("Array.prototype.{name}");
    let sentinel = vm.register_host_fn_returning_sentinel(&host_name, func);
    if let Some(obj) = vm.get_object_mut(proto_val) {
        obj.set_property(name.to_string(), sentinel);
    }
}

pub fn install_array(vm: &mut Vm) {
    vm.register_host_fn("Array.isArray", |vm, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(obj) = vm.get_object(val) {
            Ok(JsValue::from_bool(matches!(obj.kind(), ObjectKind::Array { .. })))
        } else {
            Ok(JsValue::from_bool(false))
        }
    });

    vm.register_host_fn("Array.from", array_from);

    let proto = JsObject::new();
    let proto_val = vm.alloc_object(proto);

    install_proto_method(vm, proto_val, "push", array_push);
    install_proto_method(vm, proto_val, "pop", array_pop);
    install_proto_method(vm, proto_val, "shift", array_shift);
    install_proto_method(vm, proto_val, "unshift", array_unshift);
    install_proto_method(vm, proto_val, "indexOf", array_index_of);
    install_proto_method(vm, proto_val, "includes", array_includes);
    install_proto_method(vm, proto_val, "slice", array_slice);
    install_proto_method(vm, proto_val, "concat", array_concat);
    install_proto_method(vm, proto_val, "join", array_join);
    install_proto_method(vm, proto_val, "reverse", array_reverse);
    install_proto_method(vm, proto_val, "map", array_map);
    install_proto_method(vm, proto_val, "filter", array_filter);
    install_proto_method(vm, proto_val, "reduce", array_reduce);
    install_proto_method(vm, proto_val, "forEach", array_for_each);
    install_proto_method(vm, proto_val, "find", array_find);
    install_proto_method(vm, proto_val, "some", array_some);
    install_proto_method(vm, proto_val, "every", array_every);

    vm.set_array_prototype(proto_val);

    let array_global = vm.get_global("Array");
    if let Some(obj) = vm.get_object_mut(array_global) {
        obj.set_property("prototype".to_string(), proto_val);
    }
}
