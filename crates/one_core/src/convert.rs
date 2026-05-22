use crate::{JsValue, OneError, OneResult};

/// Convert a JsValue to a Rust type
pub trait FromJs: Sized {
    fn from_js(value: JsValue) -> OneResult<Self>;
}

/// Convert a Rust type to a JsValue
pub trait IntoJs {
    fn into_js(self) -> JsValue;
}

impl FromJs for f64 {
    fn from_js(value: JsValue) -> OneResult<Self> {
        Ok(value.to_number())
    }
}

impl IntoJs for f64 {
    fn into_js(self) -> JsValue {
        JsValue::from_f64(self)
    }
}

impl FromJs for i32 {
    fn from_js(value: JsValue) -> OneResult<Self> {
        if let Some(i) = value.as_i32() {
            Ok(i)
        } else {
            Ok(value.to_number() as i32)
        }
    }
}

impl IntoJs for i32 {
    fn into_js(self) -> JsValue {
        JsValue::from_i32(self)
    }
}

impl FromJs for bool {
    fn from_js(value: JsValue) -> OneResult<Self> {
        value
            .as_bool()
            .ok_or_else(|| OneError::TypeError("expected boolean".into()))
    }
}

impl IntoJs for bool {
    fn into_js(self) -> JsValue {
        JsValue::from_bool(self)
    }
}

impl FromJs for JsValue {
    fn from_js(value: JsValue) -> OneResult<Self> {
        Ok(value)
    }
}

impl IntoJs for JsValue {
    fn into_js(self) -> JsValue {
        self
    }
}

impl IntoJs for () {
    fn into_js(self) -> JsValue {
        JsValue::undefined()
    }
}

impl<T: FromJs> FromJs for Option<T> {
    fn from_js(value: JsValue) -> OneResult<Self> {
        if value.is_null() || value.is_undefined() {
            Ok(None)
        } else {
            Ok(Some(T::from_js(value)?))
        }
    }
}

impl<T: IntoJs> IntoJs for Option<T> {
    fn into_js(self) -> JsValue {
        match self {
            Some(v) => v.into_js(),
            None => JsValue::null(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_js_i32() {
        let val = JsValue::from_i32(42);
        assert_eq!(i32::from_js(val).unwrap(), 42);
    }

    #[test]
    fn from_js_f64() {
        let val = JsValue::from_f64(3.14);
        assert!((f64::from_js(val).unwrap() - 3.14).abs() < 0.001);
    }

    #[test]
    fn from_js_bool() {
        assert_eq!(bool::from_js(JsValue::from_bool(true)).unwrap(), true);
    }

    #[test]
    fn from_js_option_none() {
        let val = JsValue::null();
        assert_eq!(Option::<i32>::from_js(val).unwrap(), None);
    }

    #[test]
    fn from_js_option_some() {
        let val = JsValue::from_i32(42);
        assert_eq!(Option::<i32>::from_js(val).unwrap(), Some(42));
    }
}
