use std::collections::HashMap;

use one_compiler::CodeBlock;
use one_core::JsValue;
use one_gc::Trace;

#[derive(Debug)]
pub struct JsObject {
    /// Properties stored as a simple HashMap (Shape system comes later)
    properties: HashMap<String, Property>,
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
pub enum ObjectKind {
    Ordinary,
    Array { length: u32 },
    Function(FunctionObject),
    HostObject { name: String },
    Promise(PromiseState),
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

impl Trace for JsObject {
    fn trace(&self, tracer: &mut dyn one_gc::trace::Tracer) {
        for prop in self.properties.values() {
            if prop.value.is_object()
                && let Some(ptr) = prop.value.as_object_raw()
            {
                tracer.mark(ptr as *const u8);
            }
        }
        if let Some(proto) = self.prototype {
            tracer.mark(proto as *const u8);
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
            properties: HashMap::new(),
            prototype: None,
            kind: ObjectKind::Ordinary,
        }
    }

    pub fn with_kind(kind: ObjectKind) -> Self {
        JsObject {
            properties: HashMap::new(),
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
        if let Some(prop) = self.properties.get(key) {
            return Some(prop.value);
        }
        if let Some(proto) = self.prototype {
            unsafe { &*proto }.get_property(key)
        } else {
            None
        }
    }

    pub fn set_property(&mut self, key: String, value: JsValue) {
        if let Some(prop) = self.properties.get_mut(&key) {
            if prop.writable {
                prop.value = value;
            }
        } else {
            self.properties.insert(key, Property::data(value));
        }
    }

    pub fn define_property(&mut self, key: String, prop: Property) {
        self.properties.insert(key, prop);
    }

    pub fn has_own_property(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }

    pub fn delete_property(&mut self, key: &str) -> bool {
        if let Some(prop) = self.properties.get(key) {
            if prop.configurable {
                self.properties.remove(key);
                return true;
            }
            return false;
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
        self.properties.keys().cloned().collect()
    }

    pub fn enumerable_keys(&self) -> Vec<String> {
        self.properties
            .iter()
            .filter(|(_, p)| p.enumerable)
            .map(|(k, _)| k.clone())
            .collect()
    }
}
