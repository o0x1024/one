use std::collections::HashMap;
use std::sync::Arc;

use one_compiler::CodeBlock;
use one_core::JsValue;
use one_gc::Trace;

use crate::shape::Shape;

const HOST_SENTINEL_MASK: u64 = 0xDEAD_0000;
const PROMISE_METHOD_MASK: u64 = 0xBEEF_0000;
const PROMISE_RESOLVER_MASK: u64 = 0xCAFE_0000;

fn is_gc_object(raw: u64) -> bool {
    raw & 0xFFFF_0000 != HOST_SENTINEL_MASK
        && raw & 0xFFFF_0000 != PROMISE_METHOD_MASK
        && raw & 0xFFFF_0000 != PROMISE_RESOLVER_MASK
}

#[derive(Debug)]
pub struct JsObject {
    /// Shape (hidden class) — describes property layout
    shape: Arc<Shape>,
    /// Inline property values — indexed by shape's slot indices
    inline_values: Vec<JsValue>,
    /// Overflow HashMap for rare cases (deleted properties, computed keys, custom descriptors)
    overflow: Option<HashMap<String, Property>>,
    /// Prototype chain link
    prototype: Option<*mut JsObject>,
    /// Object kind — distinguishes plain objects, functions, arrays, etc.
    kind: ObjectKind,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub value: JsValue,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

#[derive(Debug, Clone)]
pub struct MapData {
    pub entries: Vec<(JsValue, JsValue)>,
}

#[derive(Debug, Clone)]
pub struct SetData {
    pub values: Vec<JsValue>,
}

#[derive(Debug, Clone)]
pub enum ObjectKind {
    Ordinary,
    Array { length: u32 },
    Map(MapData),
    Set(SetData),
    Function(FunctionObject),
    HostObject { name: String },
    Promise(PromiseState),
    Date(f64),
    RegExp { pattern: String, flags: String },
}

#[derive(Debug, Clone)]
pub enum PromiseState {
    Pending {
        on_fulfilled: Vec<JsValue>,
        on_rejected: Vec<JsValue>,
    },
    Fulfilled(JsValue),
    Rejected(JsValue),
}

/// Represents a JS function with its compiled code and captured environment
#[derive(Debug, Clone)]
pub struct FunctionObject {
    pub name: String,
    pub code: CodeBlock,
    pub upvalues: Vec<JsValue>,
    pub param_count: u16,
}

fn trace_js_value(val: JsValue, tracer: &mut dyn one_gc::trace::Tracer) {
    if val.is_object()
        && let Some(raw) = val.as_object_raw()
        && is_gc_object(raw)
    {
        tracer.mark(raw as *const u8);
    }
}

impl Trace for JsObject {
    fn trace(&self, tracer: &mut dyn one_gc::trace::Tracer) {
        for val in &self.inline_values {
            trace_js_value(*val, tracer);
        }
        if let Some(ref overflow) = self.overflow {
            for prop in overflow.values() {
                trace_js_value(prop.value, tracer);
            }
        }
        if let Some(proto) = self.prototype
            && is_gc_object(proto as u64)
        {
            tracer.mark(proto as *const u8);
        }
        if let ObjectKind::Map(data) = &self.kind {
            for (key, value) in &data.entries {
                for val in [key, value] {
                    trace_js_value(*val, tracer);
                }
            }
        }
        if let ObjectKind::Set(data) = &self.kind {
            for val in &data.values {
                trace_js_value(*val, tracer);
            }
        }
        if let ObjectKind::Promise(state) = &self.kind {
            match state {
                PromiseState::Pending {
                    on_fulfilled,
                    on_rejected,
                } => {
                    for callback in on_fulfilled.iter().chain(on_rejected) {
                        trace_js_value(*callback, tracer);
                    }
                }
                PromiseState::Fulfilled(val) | PromiseState::Rejected(val) => {
                    trace_js_value(*val, tracer);
                }
            }
        }
    }
}

impl Property {
    pub fn data(value: JsValue) -> Self {
        Property {
            value,
            writable: true,
            enumerable: true,
            configurable: true,
        }
    }

    pub fn readonly(value: JsValue) -> Self {
        Property {
            value,
            writable: false,
            enumerable: true,
            configurable: false,
        }
    }

    pub fn hidden(value: JsValue) -> Self {
        Property {
            value,
            writable: true,
            enumerable: false,
            configurable: true,
        }
    }
}

impl Default for JsObject {
    fn default() -> Self {
        Self::new()
    }
}

impl JsObject {
    pub fn new() -> Self {
        JsObject {
            shape: Shape::empty(),
            inline_values: Vec::new(),
            overflow: None,
            prototype: None,
            kind: ObjectKind::Ordinary,
        }
    }

    pub fn with_kind(kind: ObjectKind) -> Self {
        JsObject {
            shape: Shape::empty(),
            inline_values: Vec::new(),
            overflow: None,
            prototype: None,
            kind,
        }
    }

    pub(crate) fn with_shared_shape(shape: Arc<Shape>, kind: ObjectKind) -> Self {
        JsObject {
            shape,
            inline_values: Vec::new(),
            overflow: None,
            prototype: None,
            kind,
        }
    }

    pub fn get_property(&self, key: &str) -> Option<JsValue> {
        if key == "length"
            && let ObjectKind::Array { length } = &self.kind
        {
            return Some(JsValue::from_i32(*length as i32));
        }
        if key == "size" {
            if let ObjectKind::Map(data) = &self.kind {
                return Some(JsValue::from_i32(data.entries.len() as i32));
            }
            if let ObjectKind::Set(data) = &self.kind {
                return Some(JsValue::from_i32(data.values.len() as i32));
            }
        }
        if let Some(ref overflow) = self.overflow
            && let Some(prop) = overflow.get(key)
        {
            return Some(prop.value);
        }
        if let Some(slot) = self.shape.lookup(key) {
            return Some(self.inline_values[slot as usize]);
        }
        if let Some(proto) = self.prototype {
            unsafe { &*proto }.get_property(key)
        } else {
            None
        }
    }

    pub fn set_property(&mut self, key: String, value: JsValue) {
        if let Some(ref mut overflow) = self.overflow
            && let Some(prop) = overflow.get_mut(&key)
        {
            if prop.writable {
                prop.value = value;
            }
            return;
        }
        if let Some(slot) = self.shape.lookup(&key) {
            let attrs = self.shape.attributes(slot);
            if attrs.writable {
                self.inline_values[slot as usize] = value;
            }
            return;
        }
        let new_shape = self.shape.transition(&key);
        self.shape = new_shape;
        self.inline_values.push(value);
    }

    pub fn define_property(&mut self, key: String, prop: Property) {
        if self.shape.lookup(&key).is_some() || self.overflow.is_some() {
            self.deopt_to_overflow();
        }
        self.overflow
            .get_or_insert_with(HashMap::new)
            .insert(key, prop);
    }

    pub fn has_own_property(&self, key: &str) -> bool {
        if self.overflow.as_ref().is_some_and(|o| o.contains_key(key)) {
            return true;
        }
        self.shape.lookup(key).is_some()
    }

    pub fn delete_property(&mut self, key: &str) -> bool {
        if let Some(ref mut overflow) = self.overflow {
            if let Some(prop) = overflow.get(key) {
                if prop.configurable {
                    overflow.remove(key);
                    return true;
                }
                return false;
            }
            return true;
        }
        if let Some(slot) = self.shape.lookup(key) {
            let attrs = self.shape.attributes(slot);
            if !attrs.configurable {
                return false;
            }
            self.deopt_to_overflow();
            if let Some(ref mut overflow) = self.overflow {
                overflow.remove(key);
            }
            return true;
        }
        true
    }

    pub fn set_prototype(&mut self, proto: Option<*mut JsObject>) {
        self.prototype = proto;
    }

    pub fn prototype(&self) -> Option<*mut JsObject> {
        self.prototype
    }

    pub fn kind(&self) -> &ObjectKind {
        &self.kind
    }

    pub fn kind_mut(&mut self) -> &mut ObjectKind {
        &mut self.kind
    }

    pub fn property_keys(&self) -> Vec<String> {
        let mut keys = self.shape.property_names().to_vec();
        if let Some(ref overflow) = self.overflow {
            for key in overflow.keys() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
        keys
    }

    pub fn enumerable_keys(&self) -> Vec<String> {
        let mut keys = self.shape.enumerable_keys();
        if let Some(ref overflow) = self.overflow {
            for (key, prop) in overflow {
                if prop.enumerable && !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
        keys
    }

    pub fn freeze(&mut self) {
        self.deopt_to_overflow();
        if let Some(ref mut overflow) = self.overflow {
            for prop in overflow.values_mut() {
                prop.writable = false;
                prop.configurable = false;
            }
        }
    }

    pub fn own_property_values(&self) -> Vec<JsValue> {
        let mut values = Vec::new();
        for (i, _name) in self.shape.property_names().iter().enumerate() {
            let attrs = self.shape.attributes(i as u32);
            if attrs.enumerable {
                values.push(self.inline_values[i]);
            }
        }
        if let Some(ref overflow) = self.overflow {
            for prop in overflow.values() {
                if prop.enumerable {
                    values.push(prop.value);
                }
            }
        }
        values
    }

    pub fn own_entries(&self) -> Vec<(String, JsValue)> {
        let mut entries = Vec::new();
        for (i, name) in self.shape.property_names().iter().enumerate() {
            let attrs = self.shape.attributes(i as u32);
            if attrs.enumerable {
                entries.push((name.clone(), self.inline_values[i]));
            }
        }
        if let Some(ref overflow) = self.overflow {
            for (key, prop) in overflow {
                if prop.enumerable {
                    entries.push((key.clone(), prop.value));
                }
            }
        }
        entries
    }

    /// Move all inline properties to overflow and reset shape to empty.
    fn deopt_to_overflow(&mut self) {
        if self.shape.property_count() == 0 {
            if self.overflow.is_none() {
                self.overflow = Some(HashMap::new());
            }
            return;
        }

        let mut overflow = self.overflow.take().unwrap_or_default();
        for (i, name) in self.shape.property_names().iter().enumerate() {
            let attrs = self.shape.attributes(i as u32);
            overflow.insert(
                name.clone(),
                Property {
                    value: self.inline_values[i],
                    writable: attrs.writable,
                    enumerable: attrs.enumerable,
                    configurable: attrs.configurable,
                },
            );
        }
        self.shape = Shape::empty();
        self.inline_values.clear();
        self.overflow = Some(overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use one_core::JsValue;

    #[test]
    fn object_inline_storage() {
        let mut obj = JsObject::new();
        obj.set_property("x".to_string(), JsValue::from_i32(10));
        obj.set_property("y".to_string(), JsValue::from_i32(20));
        assert_eq!(obj.get_property("x").unwrap().as_i32(), Some(10));
        assert_eq!(obj.get_property("y").unwrap().as_i32(), Some(20));
    }
}
