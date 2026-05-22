use std::collections::HashMap;

use one_core::{JsValue, OneError, OneResult};

use crate::object::{JsObject, ObjectKind};
use crate::Vm;

/// Convert from JsValue with VM context
pub trait FromJsValue: Sized {
    fn from_js_value(vm: &Vm, value: JsValue) -> OneResult<Self>;
}

/// Convert to JsValue with VM context
pub trait IntoJsValue {
    fn into_js_value(self, vm: &mut Vm) -> JsValue;
}

impl FromJsValue for String {
    fn from_js_value(vm: &Vm, value: JsValue) -> OneResult<Self> {
        Ok(vm.value_to_string(value))
    }
}

impl IntoJsValue for String {
    fn into_js_value(self, vm: &mut Vm) -> JsValue {
        vm.alloc_string(self)
    }
}

impl IntoJsValue for &str {
    fn into_js_value(self, vm: &mut Vm) -> JsValue {
        vm.alloc_string(self.to_string())
    }
}

impl<T: FromJsValue> FromJsValue for Vec<T> {
    fn from_js_value(vm: &Vm, value: JsValue) -> OneResult<Self> {
        if let Some(obj) = vm.get_object(value)
            && let ObjectKind::Array { length } = obj.kind()
        {
            let mut result = Vec::with_capacity(*length as usize);
            for i in 0..*length {
                let elem = obj
                    .get_property(&i.to_string())
                    .unwrap_or(JsValue::undefined());
                result.push(T::from_js_value(vm, elem)?);
            }
            return Ok(result);
        }
        Err(OneError::TypeError("expected array".into()))
    }
}

impl<T: IntoJsValue> IntoJsValue for Vec<T> {
    fn into_js_value(self, vm: &mut Vm) -> JsValue {
        let len = self.len() as u32;
        let arr = JsObject::with_kind(ObjectKind::Array { length: len });
        let arr_val = vm.alloc_object(arr);
        for (i, item) in self.into_iter().enumerate() {
            let val = item.into_js_value(vm);
            if let Some(obj) = vm.get_object_mut(arr_val) {
                obj.set_property(i.to_string(), val);
            }
        }
        arr_val
    }
}

impl<T: FromJsValue> FromJsValue for HashMap<String, T> {
    fn from_js_value(vm: &Vm, value: JsValue) -> OneResult<Self> {
        if let Some(obj) = vm.get_object(value) {
            let mut map = HashMap::new();
            for key in obj.enumerable_keys() {
                let val = obj.get_property(&key).unwrap_or(JsValue::undefined());
                map.insert(key, T::from_js_value(vm, val)?);
            }
            Ok(map)
        } else {
            Err(OneError::TypeError("expected object".into()))
        }
    }
}

impl<T: IntoJsValue> IntoJsValue for HashMap<String, T> {
    fn into_js_value(self, vm: &mut Vm) -> JsValue {
        let obj = JsObject::new();
        let obj_val = vm.alloc_object(obj);
        for (key, value) in self {
            let val = value.into_js_value(vm);
            if let Some(obj) = vm.get_object_mut(obj_val) {
                obj.set_property(key, val);
            }
        }
        obj_val
    }
}
