use one_core::JsValue;
use one_vm::Vm;

pub(crate) fn parse_int_string(s: &str, radix: i32) -> f64 {
    let trimmed = s.trim_start();
    if trimmed.is_empty() {
        return f64::NAN;
    }

    let mut radix = if radix == 0 { 10 } else { radix };
    let mut s = trimmed;
    let mut sign = 1.0;

    if let Some(rest) = s.strip_prefix('+') {
        s = rest;
    } else if let Some(rest) = s.strip_prefix('-') {
        sign = -1.0;
        s = rest;
    }

    if radix == 0 || radix == 16 {
        if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            s = rest;
            radix = 16;
        } else if radix == 0 {
            radix = 10;
        }
    }

    if !(2..=36).contains(&radix) {
        return f64::NAN;
    }

    let mut result = 0.0;
    let mut found = false;
    for c in s.chars() {
        let digit = match c {
            '0'..='9' => c as u32 - '0' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 10,
            'A'..='Z' => c as u32 - 'A' as u32 + 10,
            _ => break,
        };
        if digit >= radix as u32 {
            break;
        }
        found = true;
        result = result * f64::from(radix) + f64::from(digit);
    }

    if found {
        sign * result
    } else {
        f64::NAN
    }
}

pub(crate) fn parse_float_string(s: &str) -> f64 {
    let trimmed = s.trim_start();
    if trimmed.is_empty() {
        return f64::NAN;
    }

    if trimmed.starts_with("Infinity") || trimmed.starts_with("+Infinity") {
        return f64::INFINITY;
    }
    if trimmed.starts_with("-Infinity") {
        return f64::NEG_INFINITY;
    }
    if trimmed.starts_with("NaN")
        || trimmed.starts_with("+NaN")
        || trimmed.starts_with("-NaN")
    {
        return f64::NAN;
    }

    let bytes = trimmed.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }

    let mut saw_digit = false;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        saw_digit = true;
        end += 1;
    }

    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            saw_digit = true;
            end += 1;
        }
    }

    if !saw_digit {
        return f64::NAN;
    }

    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let exp_start = end;
        end += 1;
        if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
            end += 1;
        }
        let digit_start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == digit_start {
            end = exp_start;
        }
    }

    trimmed[..end].parse::<f64>().unwrap_or(f64::NAN)
}

fn number_to_string(n: f64, radix: i32) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n.is_sign_positive() {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }

    let radix = if radix == 0 { 10 } else { radix };
    if !(2..=36).contains(&radix) {
        return "NaN".to_string();
    }

    if radix == 10 {
        return format!("{n}");
    }

    let negative = n < 0.0;
    let mut value = n.abs().floor();
    if value == 0.0 {
        return if negative {
            "-0".to_string()
        } else {
            "0".to_string()
        };
    }

    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut chars = Vec::new();
    while value > 0.0 {
        let rem = (value % f64::from(radix)).trunc() as usize;
        chars.push(digits[rem] as char);
        value = (value / f64::from(radix)).floor();
    }
    chars.reverse();
    let mut result = String::new();
    if negative {
        result.push('-');
    }
    result.extend(chars);
    result
}

pub fn install_number(vm: &mut Vm) {
    vm.register_host_fn("Number.isFinite", |_vm, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let finite = val.is_number()
            && val.to_number().is_finite()
            && !val.to_number().is_nan();
        Ok(JsValue::from_bool(finite))
    });

    vm.register_host_fn("Number.isInteger", |_vm, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if !val.is_number() {
            return Ok(JsValue::from_bool(false));
        }
        let n = val.to_number();
        Ok(JsValue::from_bool(n.is_finite() && n.fract() == 0.0))
    });

    vm.register_host_fn("Number.isNaN", |_vm, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(JsValue::from_bool(val.is_float64() && val.to_number().is_nan()))
    });

    vm.register_host_fn("Number.parseInt", |vm, args| {
        let s = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        let radix = args
            .get(1)
            .map(|v| v.to_number())
            .unwrap_or(0.0)
            .trunc() as i32;
        Ok(JsValue::from_f64(parse_int_string(&s, radix)))
    });

    vm.register_host_fn("Number.parseFloat", |vm, args| {
        let s = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        Ok(JsValue::from_f64(parse_float_string(&s)))
    });

    vm.register_host_fn("Number.prototype.toFixed", |vm, args| {
        let this = vm.get_global("this");
        let n = this.to_number();
        let digits = args
            .first()
            .map(|v| v.to_number())
            .unwrap_or(0.0)
            .trunc() as i32;
        let digits = digits.clamp(0, 100);
        Ok(vm.alloc_string(format!("{n:.digits$}", digits = digits as usize)))
    });

    vm.register_host_fn("Number.prototype.toString", |vm, args| {
        let this = vm.get_global("this");
        let n = this.to_number();
        let radix = args
            .first()
            .map(|v| v.to_number())
            .unwrap_or(10.0)
            .trunc() as i32;
        Ok(vm.alloc_string(number_to_string(n, radix)))
    });
}
