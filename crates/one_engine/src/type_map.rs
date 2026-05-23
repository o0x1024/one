use std::any::{Any, TypeId};
use std::collections::HashMap;

#[derive(Default)]
pub struct TypeMap {
    map: HashMap<TypeId, Box<dyn Any + Send>>,
}

impl TypeMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Send + 'static>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get<T: Send + 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref())
    }

    pub fn get_mut<T: Send + 'static>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut())
    }

    pub fn contains<T: Send + 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: Send + 'static>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast().ok())
            .map(|b| *b)
    }
}
