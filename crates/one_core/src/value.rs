/// NaN-boxing 值表示
///
/// 布局：
/// - 若 (bits >> 48) < 0xFFF8 → f64（原始 IEEE 754 位模式）
/// - 若 (bits >> 48) >= 0xFFF8 → 带标签的非 f64 值
///
/// 标签分配（高 16 位）：
///   0xFFF9 = undefined
///   0xFFFA = null
///   0xFFFB = boolean (bit 0 = true/false)
///   0xFFFC = i32 (低 32 位)
///   0xFFFD = symbol
///   0xFFFE = string pointer (低 48 位)
///   0xFFFF = object pointer (低 48 位)
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JsValue(pub(crate) u64);

const CANON_NAN_BITS: u64 = 0x7FF8_0000_0000_0000;
const TAG_THRESHOLD: u64 = 0xFFF8;
const TAG_UNDEFINED: u64 = 0xFFF9_0000_0000_0000;
const TAG_NULL: u64 = 0xFFFA_0000_0000_0000;
const TAG_BOOL: u64 = 0xFFFB_0000_0000_0000;
const TAG_INT32: u64 = 0xFFFC_0000_0000_0000;
const TAG_SYMBOL: u64 = 0xFFFD_0000_0000_0000;
const TAG_STRING: u64 = 0xFFFE_0000_0000_0000;
const TAG_OBJECT: u64 = 0xFFFF_0000_0000_0000;
const TAG_MASK: u64 = 0xFFFF_0000_0000_0000;
const PTR_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

impl JsValue {
    #[inline]
    pub fn from_f64(v: f64) -> Self {
        let bits = v.to_bits();
        if bits & 0x7FF8_0000_0000_0000 == 0x7FF8_0000_0000_0000 {
            Self(CANON_NAN_BITS)
        } else {
            Self(bits)
        }
    }

    #[inline]
    pub fn is_float64(&self) -> bool {
        self.0 >> 48 < TAG_THRESHOLD
    }

    #[inline]
    pub fn is_number(&self) -> bool {
        self.is_float64() || self.is_int32()
    }

    #[inline]
    pub fn is_int32(&self) -> bool {
        self.0 & TAG_MASK == TAG_INT32
    }

    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        if self.is_float64() {
            Some(f64::from_bits(self.0))
        } else {
            None
        }
    }

    #[inline]
    pub fn from_i32(v: i32) -> Self {
        Self(TAG_INT32 | (v as u32 as u64))
    }

    #[inline]
    pub fn as_i32(&self) -> Option<i32> {
        if self.is_int32() {
            Some((self.0 & 0xFFFF_FFFF) as i32)
        } else {
            None
        }
    }

    #[inline]
    pub fn from_bool(v: bool) -> Self {
        Self(TAG_BOOL | u64::from(v))
    }

    #[inline]
    pub fn is_boolean(&self) -> bool {
        self.0 & TAG_MASK == TAG_BOOL
    }

    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        if self.is_boolean() {
            Some(self.0 & 1 != 0)
        } else {
            None
        }
    }

    #[inline]
    pub const fn null() -> Self {
        Self(TAG_NULL)
    }

    #[inline]
    pub const fn undefined() -> Self {
        Self(TAG_UNDEFINED)
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0 == TAG_NULL
    }

    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.0 == TAG_UNDEFINED
    }

    #[inline]
    pub fn is_nullish(&self) -> bool {
        self.is_null() || self.is_undefined()
    }

    #[inline]
    pub fn to_number(&self) -> f64 {
        if let Some(v) = self.as_f64() {
            v
        } else if let Some(v) = self.as_i32() {
            f64::from(v)
        } else {
            f64::NAN
        }
    }

    #[inline]
    pub fn from_object_raw(ptr: u64) -> Self {
        debug_assert!(ptr <= PTR_MASK, "object pointer must fit in 48 bits");
        Self(TAG_OBJECT | (ptr & PTR_MASK))
    }

    #[inline]
    pub fn is_object(&self) -> bool {
        self.0 & TAG_MASK == TAG_OBJECT
    }

    #[inline]
    pub fn as_object_raw(&self) -> Option<u64> {
        if self.is_object() {
            Some(self.0 & PTR_MASK)
        } else {
            None
        }
    }

    #[inline]
    pub fn from_string_raw(ptr: u64) -> Self {
        debug_assert!(ptr <= PTR_MASK, "string pointer must fit in 48 bits");
        Self(TAG_STRING | (ptr & PTR_MASK))
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        self.0 & TAG_MASK == TAG_STRING
    }

    #[inline]
    pub fn as_string_raw(&self) -> Option<u64> {
        if self.is_string() {
            Some(self.0 & PTR_MASK)
        } else {
            None
        }
    }

    #[inline]
    pub fn from_symbol_raw(id: u32) -> Self {
        Self(TAG_SYMBOL | u64::from(id))
    }

    #[inline]
    pub fn is_symbol(&self) -> bool {
        self.0 & TAG_MASK == TAG_SYMBOL
    }

    #[inline]
    pub fn as_symbol_raw(&self) -> Option<u32> {
        if self.is_symbol() {
            Some((self.0 & 0xFFFF_FFFF) as u32)
        } else {
            None
        }
    }

    #[inline]
    pub fn type_of(&self) -> &'static str {
        if self.is_float64() || self.is_int32() {
            "number"
        } else if self.is_boolean() {
            "boolean"
        } else if self.is_string() {
            "string"
        } else if self.is_symbol() {
            "symbol"
        } else if self.is_undefined() {
            "undefined"
        } else {
            // null and object both report "object" in ES typeof
            "object"
        }
    }

    #[inline]
    fn tag(&self) -> u16 {
        (self.0 >> 48) as u16
    }
}

impl PartialEq for JsValue {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for JsValue {}

impl std::fmt::Display for JsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.as_f64() {
            if v.is_nan() {
                write!(f, "NaN")
            } else if v == f64::INFINITY {
                write!(f, "Infinity")
            } else if v == f64::NEG_INFINITY {
                write!(f, "-Infinity")
            } else {
                write!(f, "{v}")
            }
        } else if let Some(v) = self.as_i32() {
            write!(f, "{v}")
        } else if let Some(v) = self.as_bool() {
            write!(f, "{v}")
        } else if self.is_null() {
            write!(f, "null")
        } else if self.is_undefined() {
            write!(f, "undefined")
        } else if let Some(ptr) = self.as_string_raw() {
            write!(f, "[string@0x{ptr:012x}]")
        } else if let Some(ptr) = self.as_object_raw() {
            write!(f, "[object@0x{ptr:012x}]")
        } else if let Some(id) = self.as_symbol_raw() {
            write!(f, "Symbol({id})")
        } else {
            write!(f, "JsValue({:#x})", self.0)
        }
    }
}

impl std::fmt::Debug for JsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsValue({} tag={:#06x} bits={:#018x})", self, self.tag(), self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::JsValue;

    #[test]
    fn f64_round_trip() {
        let cases = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            3.14,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            f64::EPSILON,
        ];
        for v in cases {
            let js = JsValue::from_f64(v);
            assert!(js.is_number());
            assert!(js.is_float64());
            assert_eq!(js.as_f64().unwrap().to_bits(), v.to_bits());
        }
    }

    #[test]
    fn nan_is_canonicalized() {
        let nan1 = JsValue::from_f64(f64::NAN);
        let nan2 = JsValue::from_f64(-f64::NAN);
        assert!(nan1.is_number());
        assert!(nan1.as_f64().unwrap().is_nan());
        assert_eq!(nan1.0, nan2.0);
    }

    #[test]
    fn negative_zero_preserved() {
        let nz = JsValue::from_f64(-0.0);
        let pz = JsValue::from_f64(0.0);
        assert!(nz.as_f64().unwrap().is_sign_negative());
        assert!(pz.as_f64().unwrap().is_sign_positive());
        assert_ne!(nz.0, pz.0);
    }

    #[test]
    fn i32_round_trip() {
        let cases = [0, 1, -1, 42, i32::MAX, i32::MIN, 1_000_000];
        for v in cases {
            let js = JsValue::from_i32(v);
            assert!(js.is_int32());
            assert!(js.is_number());
            assert!(!js.is_float64());
            assert_eq!(js.as_i32().unwrap(), v);
        }
    }

    #[test]
    fn i32_to_f64_conversion() {
        let js = JsValue::from_i32(42);
        assert_eq!(js.to_number(), 42.0);
    }

    #[test]
    fn boolean_encoding() {
        let t = JsValue::from_bool(true);
        let f = JsValue::from_bool(false);
        assert!(t.is_boolean());
        assert!(f.is_boolean());
        assert_eq!(t.as_bool().unwrap(), true);
        assert_eq!(f.as_bool().unwrap(), false);
        assert!(!t.is_number());
        assert!(!t.is_null());
        assert_ne!(t.0, f.0);
    }

    #[test]
    fn null_and_undefined() {
        let n = JsValue::null();
        let u = JsValue::undefined();
        assert!(n.is_null());
        assert!(u.is_undefined());
        assert!(n.is_nullish());
        assert!(u.is_nullish());
        assert!(!n.is_undefined());
        assert!(!u.is_null());
        assert!(!n.is_number());
        assert!(!u.is_boolean());
        assert_ne!(n.0, u.0);
    }

    #[test]
    fn object_pointer_round_trip() {
        let fake_ptr: u64 = 0x0000_1234_5678_9AB0;
        let js = JsValue::from_object_raw(fake_ptr);
        assert!(js.is_object());
        assert!(!js.is_string());
        assert!(!js.is_number());
        assert_eq!(js.as_object_raw().unwrap(), fake_ptr);
    }

    #[test]
    fn string_pointer_round_trip() {
        let fake_ptr: u64 = 0x0000_ABCD_EF01_2340;
        let js = JsValue::from_string_raw(fake_ptr);
        assert!(js.is_string());
        assert!(!js.is_object());
        assert_eq!(js.as_string_raw().unwrap(), fake_ptr);
    }

    #[test]
    fn type_of_all_types() {
        assert_eq!(JsValue::from_f64(1.0).type_of(), "number");
        assert_eq!(JsValue::from_i32(1).type_of(), "number");
        assert_eq!(JsValue::from_bool(true).type_of(), "boolean");
        assert_eq!(JsValue::null().type_of(), "object");
        assert_eq!(JsValue::undefined().type_of(), "undefined");
        assert_eq!(JsValue::from_string_raw(0x1000).type_of(), "string");
        assert_eq!(JsValue::from_object_raw(0x2000).type_of(), "object");
        assert_eq!(JsValue::from_symbol_raw(42).type_of(), "symbol");
    }

    #[test]
    fn display_formatting() {
        assert_eq!(format!("{}", JsValue::from_f64(3.14)), "3.14");
        assert_eq!(format!("{}", JsValue::from_i32(42)), "42");
        assert_eq!(format!("{}", JsValue::from_bool(true)), "true");
        assert_eq!(format!("{}", JsValue::null()), "null");
        assert_eq!(format!("{}", JsValue::undefined()), "undefined");
    }

    #[test]
    fn all_types_are_distinct() {
        let values = [
            JsValue::from_f64(0.0),
            JsValue::from_i32(0),
            JsValue::from_bool(false),
            JsValue::null(),
            JsValue::undefined(),
        ];
        for (i, a) in values.iter().enumerate() {
            for (j, b) in values.iter().enumerate() {
                if i != j {
                    assert_ne!(a.0, b.0, "type {i} and {j} should have different bit patterns");
                }
            }
        }
    }
}
