use one_core::JsValue;
use one_vm::object::{JsObject, ObjectKind};
use one_vm::Vm;
use serde_json::Value as JsonValue;

pub fn json_to_js(vm: &mut Vm, value: &JsonValue) -> JsValue {
    match value {
        JsonValue::Null => JsValue::null(),
        JsonValue::Bool(b) => JsValue::from_bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    JsValue::from_i32(i as i32)
                } else {
                    JsValue::from_f64(i as f64)
                }
            } else {
                JsValue::from_f64(n.as_f64().unwrap_or(0.0))
            }
        }
        JsonValue::String(s) => vm.alloc_string(s.clone()),
        JsonValue::Array(arr) => {
            let len = arr.len() as u32;
            let obj = JsObject::with_kind(ObjectKind::Array { length: len });
            let obj_val = vm.alloc_object(obj);
            for (i, item) in arr.iter().enumerate() {
                let val = json_to_js(vm, item);
                if let Some(obj) = vm.get_object_mut(obj_val) {
                    obj.set_property(i.to_string(), val);
                }
            }
            obj_val
        }
        JsonValue::Object(map) => {
            let obj = JsObject::new();
            let obj_val = vm.alloc_object(obj);
            for (key, value) in map {
                let val = json_to_js(vm, value);
                if let Some(obj) = vm.get_object_mut(obj_val) {
                    obj.set_property(key.clone(), val);
                }
            }
            obj_val
        }
    }
}

pub fn js_to_json(vm: &Vm, value: JsValue) -> JsonValue {
    if value.is_null() || value.is_undefined() {
        return JsonValue::Null;
    }
    if value.is_boolean() {
        return JsonValue::Bool(value.as_bool().unwrap());
    }
    if value.is_int32() {
        return JsonValue::Number(value.as_i32().unwrap().into());
    }
    if value.is_float64() {
        let n = value.as_f64().unwrap();
        if let Some(num) = serde_json::Number::from_f64(n) {
            return JsonValue::Number(num);
        }
        return JsonValue::Null;
    }
    if value.is_string() {
        return JsonValue::String(vm.value_to_string(value));
    }
    if let Some(obj) = vm.get_object(value) {
        match obj.kind() {
            ObjectKind::Array { length } => {
                let mut arr = Vec::with_capacity(*length as usize);
                for i in 0..*length {
                    let elem = obj
                        .get_property(&i.to_string())
                        .unwrap_or(JsValue::undefined());
                    arr.push(js_to_json(vm, elem));
                }
                JsonValue::Array(arr)
            }
            _ => {
                let mut map = serde_json::Map::new();
                for key in obj.enumerable_keys() {
                    if let Some(val) = obj.get_property(&key) {
                        map.insert(key, js_to_json(vm, val));
                    }
                }
                JsonValue::Object(map)
            }
        }
    } else {
        JsonValue::Null
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    #[test]
    fn json_to_js_number() {
        let mut engine = Engine::new();
        let json = serde_json::json!(42);
        engine.set_json_global("x", &json);
        let result = engine.eval("return x;").unwrap();
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn json_to_js_object() {
        let mut engine = Engine::new();
        let json = serde_json::json!({"name": "Alice", "age": 30});
        engine.set_json_global("person", &json);
        let result = engine.eval("return person.age;").unwrap();
        assert!(result.to_number() == 30.0);
    }

    #[test]
    fn json_to_js_array() {
        let mut engine = Engine::new();
        let json = serde_json::json!([1, 2, 3]);
        engine.set_json_global("arr", &json);
        let result = engine.eval("return arr.length;").unwrap();
        assert!(result.to_number() == 3.0);
    }

    #[test]
    fn js_to_json_object() {
        let mut engine = Engine::new();
        engine
            .eval("result = {a: 1, b: 'hello', c: true};")
            .unwrap();
        let json = engine.get_json_global("result");
        assert_eq!(json["a"], 1);
        assert_eq!(json["b"], "hello");
        assert_eq!(json["c"], true);
    }

    #[test]
    fn js_to_json_array() {
        let mut engine = Engine::new();
        engine.eval("result = [1, 2, 3];").unwrap();
        let json = engine.get_json_global("result");
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 3);
    }

    #[test]
    fn eval_to_json() {
        let mut engine = Engine::new();
        let json = engine
            .eval_to_json("return {x: 1, y: [2, 3]};")
            .unwrap();
        assert_eq!(json["x"], 1);
        assert!(json["y"].is_array());
    }

    #[test]
    fn serde_round_trip() {
        let mut engine = Engine::new();
        let input = serde_json::json!({
            "name": "test",
            "values": [1, 2, 3],
            "nested": {"a": true, "b": null}
        });
        engine.set_json_global("data", &input);
        let output = engine.get_json_global("data");
        assert_eq!(input, output);
    }
}
