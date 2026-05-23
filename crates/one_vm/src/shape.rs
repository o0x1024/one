use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy)]
pub struct PropertyAttributes {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

impl Default for PropertyAttributes {
    fn default() -> Self {
        PropertyAttributes {
            writable: true,
            enumerable: true,
            configurable: true,
        }
    }
}

/// Shape describes the property layout of an object
#[derive(Debug)]
pub struct Shape {
    /// Map from property name to slot index
    property_map: HashMap<String, u32>,
    /// Ordered property names (for enumeration)
    property_names: Vec<String>,
    /// Transition table: property name → child shape
    transitions: Mutex<HashMap<String, Arc<Shape>>>,
    /// Property attributes for each slot
    attributes: Vec<PropertyAttributes>,
}

static ROOT_SHAPE: OnceLock<Arc<Shape>> = OnceLock::new();

impl Shape {
    /// The empty root shape (singleton — shared by all new objects)
    pub fn empty() -> Arc<Self> {
        ROOT_SHAPE
            .get_or_init(|| {
                Arc::new(Shape {
                    property_map: HashMap::new(),
                    property_names: Vec::new(),
                    transitions: Mutex::new(HashMap::new()),
                    attributes: Vec::new(),
                })
            })
            .clone()
    }

    /// Number of properties in this shape
    pub fn property_count(&self) -> u32 {
        self.property_names.len() as u32
    }

    /// Look up a property's slot index
    pub fn lookup(&self, name: &str) -> Option<u32> {
        self.property_map.get(name).copied()
    }

    /// Get or create a child shape by adding a property
    pub fn transition(&self, name: &str) -> Arc<Shape> {
        {
            let transitions = self.transitions.lock().unwrap();
            if let Some(child) = transitions.get(name) {
                return child.clone();
            }
        }

        let mut new_map = self.property_map.clone();
        let idx = self.property_names.len() as u32;
        new_map.insert(name.to_string(), idx);

        let mut new_names = self.property_names.clone();
        new_names.push(name.to_string());

        let mut new_attrs = self.attributes.clone();
        new_attrs.push(PropertyAttributes::default());

        let child = Arc::new(Shape {
            property_map: new_map,
            property_names: new_names,
            transitions: Mutex::new(HashMap::new()),
            attributes: new_attrs,
        });

        self.transitions
            .lock()
            .unwrap()
            .insert(name.to_string(), child.clone());

        child
    }

    /// Get ordered property names
    pub fn property_names(&self) -> &[String] {
        &self.property_names
    }

    /// Get enumerable property names
    pub fn enumerable_keys(&self) -> Vec<String> {
        self.property_names
            .iter()
            .enumerate()
            .filter(|(i, _)| self.attributes[*i].enumerable)
            .map(|(_, n)| n.clone())
            .collect()
    }

    /// Get attributes for a slot
    pub fn attributes(&self, slot: u32) -> PropertyAttributes {
        self.attributes[slot as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_basic() {
        let shape = Shape::empty();
        assert_eq!(shape.property_count(), 0);
        assert!(shape.lookup("x").is_none());
    }

    #[test]
    fn shape_transition() {
        let shape = Shape::empty();
        let shape2 = shape.transition("x");
        assert_eq!(shape2.property_count(), 1);
        assert_eq!(shape2.lookup("x"), Some(0));

        let shape3 = shape2.transition("y");
        assert_eq!(shape3.property_count(), 2);
        assert_eq!(shape3.lookup("y"), Some(1));
    }

    #[test]
    fn shape_transition_caching() {
        let shape = Shape::empty();
        let a1 = shape.transition("x");
        let a2 = shape.transition("x");
        assert!(Arc::ptr_eq(&a1, &a2));
    }
}
