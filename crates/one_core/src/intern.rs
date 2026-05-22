use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InternId(pub u32);

pub struct StringInterner {
    map: HashMap<String, InternId>,
    strings: Vec<String>,
}

pub const WELL_KNOWN_UNDEFINED: InternId = InternId(0);
pub const WELL_KNOWN_NULL: InternId = InternId(1);
pub const WELL_KNOWN_TRUE: InternId = InternId(2);
pub const WELL_KNOWN_FALSE: InternId = InternId(3);
pub const WELL_KNOWN_LENGTH: InternId = InternId(4);
pub const WELL_KNOWN_PROTOTYPE: InternId = InternId(5);
pub const WELL_KNOWN_CONSTRUCTOR: InternId = InternId(6);
pub const WELL_KNOWN___PROTO__: InternId = InternId(7);
pub const WELL_KNOWN_TO_STRING: InternId = InternId(8);
pub const WELL_KNOWN_VALUE_OF: InternId = InternId(9);
pub const WELL_KNOWN_HAS_INSTANCE: InternId = InternId(10);
pub const WELL_KNOWN_ITERATOR: InternId = InternId(11);

const WELL_KNOWN_STRINGS: &[&str] = &[
    "undefined", "null", "true", "false", "length", "prototype", "constructor", "__proto__",
    "toString", "valueOf", "hasInstance", "iterator",
];

impl StringInterner {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    pub fn with_well_known() -> Self {
        let mut interner = Self::new();
        for s in WELL_KNOWN_STRINGS {
            interner.intern(s);
        }
        interner
    }

    pub fn intern(&mut self, s: &str) -> InternId {
        if let Some(&id) = self.map.get(s) {
            return id;
        }
        let id = InternId(u32::try_from(self.strings.len()).expect("interner overflow"));
        self.map.insert(s.to_string(), id);
        self.strings.push(s.to_string());
        id
    }

    pub fn resolve(&self, id: InternId) -> Option<&str> {
        self.strings.get(id.0 as usize).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::with_well_known()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_id() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("hello");
        assert_eq!(a, b);
    }

    #[test]
    fn different_strings_get_different_ids() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("world");
        assert_ne!(a, b);
    }

    #[test]
    fn resolve_returns_original_string() {
        let mut interner = StringInterner::new();
        let id = interner.intern("hello");
        assert_eq!(interner.resolve(id), Some("hello"));
    }

    #[test]
    fn resolve_unknown_returns_none() {
        let interner = StringInterner::new();
        assert_eq!(interner.resolve(InternId(9999)), None);
    }

    #[test]
    fn well_known_strings_pre_interned() {
        let interner = StringInterner::with_well_known();
        assert!(interner.resolve(WELL_KNOWN_UNDEFINED).is_some());
        assert_eq!(interner.resolve(WELL_KNOWN_UNDEFINED), Some("undefined"));
        assert_eq!(interner.resolve(WELL_KNOWN_NULL), Some("null"));
        assert_eq!(interner.resolve(WELL_KNOWN_TRUE), Some("true"));
        assert_eq!(interner.resolve(WELL_KNOWN_FALSE), Some("false"));
        assert_eq!(interner.resolve(WELL_KNOWN_LENGTH), Some("length"));
        assert_eq!(interner.resolve(WELL_KNOWN_PROTOTYPE), Some("prototype"));
        assert_eq!(interner.resolve(WELL_KNOWN_CONSTRUCTOR), Some("constructor"));
    }

    #[test]
    fn intern_returns_well_known_id_for_known_strings() {
        let mut interner = StringInterner::with_well_known();
        let id = interner.intern("length");
        assert_eq!(id, WELL_KNOWN_LENGTH);
    }

    #[test]
    fn many_strings() {
        let mut interner = StringInterner::new();
        let ids: Vec<_> = (0..1000)
            .map(|i| interner.intern(&format!("str_{i}")))
            .collect();
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(interner.resolve(*id), Some(format!("str_{i}").as_str()));
        }
    }
}
