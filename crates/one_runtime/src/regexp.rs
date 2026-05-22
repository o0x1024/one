use regex::Regex;
use one_core::JsValue;
use one_vm::Vm;
use one_vm::object::{JsObject, ObjectKind};

pub fn regexp_pattern_flags(vm: &Vm, val: JsValue) -> Option<(String, String)> {
    vm.get_object(val).and_then(|obj| {
        if let ObjectKind::RegExp { pattern, flags } = obj.kind() {
            Some((pattern.clone(), flags.clone()))
        } else {
            None
        }
    })
}

fn compile_regex(pattern: &str) -> Option<Regex> {
    Regex::new(pattern).ok()
}

fn make_match_array(vm: &mut Vm, matched: &str, index: usize, input: &str) -> JsValue {
    let matched_val = vm.alloc_string(matched.to_string());
    let input_val = vm.alloc_string(input.to_string());
    let arr = vm.new_array(1);
    if let Some(obj) = vm.get_object_mut(arr) {
        obj.set_property("0".to_string(), matched_val);
        obj.set_property("index".to_string(), JsValue::from_i32(index as i32));
        obj.set_property("input".to_string(), input_val);
    }
    arr
}

pub fn regexp_match(vm: &mut Vm, input: &str, pattern: &str, _flags: &str) -> Option<JsValue> {
    let re = compile_regex(pattern)?;
    let mat = re.find(input)?;
    Some(make_match_array(vm, mat.as_str(), mat.start(), input))
}

pub fn regexp_search(input: &str, pattern: &str) -> i32 {
    compile_regex(pattern)
        .and_then(|re| re.find(input).map(|m| m.start() as i32))
        .unwrap_or(-1)
}

fn regexp_constructor(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let pattern = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let flags = args.get(1).map(|v| vm.value_to_string(*v)).unwrap_or_default();
    Ok(vm.new_regexp(pattern, flags))
}

fn regexp_test(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let input = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let matched = regexp_pattern_flags(vm, this)
        .and_then(|(pattern, _flags)| compile_regex(&pattern).map(|re| re.is_match(&input)))
        .unwrap_or(false);
    Ok(JsValue::from_bool(matched))
}

fn regexp_exec(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let this = vm.get_global("this");
    let input = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let result = regexp_pattern_flags(vm, this).and_then(|(pattern, flags)| {
        regexp_match(vm, &input, &pattern, &flags)
    });
    Ok(result.unwrap_or(JsValue::null()))
}

fn install_proto_method<F>(vm: &mut Vm, proto_val: JsValue, name: &str, func: F)
where
    F: Fn(&mut Vm, &[JsValue]) -> one_core::OneResult<JsValue> + 'static,
{
    let host_name = format!("RegExp.prototype.{name}");
    let sentinel = vm.register_host_fn_returning_sentinel(&host_name, func);
    if let Some(obj) = vm.get_object_mut(proto_val) {
        obj.set_property(name.to_string(), sentinel);
    }
}

pub fn install_regexp(vm: &mut Vm) {
    vm.register_host_fn("RegExp", regexp_constructor);

    let proto = JsObject::new();
    let proto_val = vm.alloc_object(proto);

    install_proto_method(vm, proto_val, "test", regexp_test);
    install_proto_method(vm, proto_val, "exec", regexp_exec);

    vm.set_regexp_prototype(proto_val);

    let regexp_global = vm.get_global("RegExp");
    if let Some(obj) = vm.get_object_mut(regexp_global) {
        obj.set_property("prototype".to_string(), proto_val);
    }

    vm.register_host_fn("String.prototype.match", |vm, args| {
        let this = vm.get_global("this");
        let input = vm.value_to_string(this);
        let regexp = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some((pattern, flags)) = regexp_pattern_flags(vm, regexp) {
            Ok(regexp_match(vm, &input, &pattern, &flags).unwrap_or(JsValue::null()))
        } else {
            Ok(JsValue::null())
        }
    });

    vm.register_host_fn("String.prototype.search", |vm, args| {
        let this = vm.get_global("this");
        let input = vm.value_to_string(this);
        let regexp = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some((pattern, _flags)) = regexp_pattern_flags(vm, regexp) {
            Ok(JsValue::from_i32(regexp_search(&input, &pattern)))
        } else {
            Ok(JsValue::from_i32(-1))
        }
    });
}
