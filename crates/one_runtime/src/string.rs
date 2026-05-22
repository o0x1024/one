use one_core::JsValue;
use one_vm::Vm;

fn arg_string(vm: &Vm, args: &[JsValue], index: usize) -> String {
    args.get(index)
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default()
}

fn arg_number(args: &[JsValue], index: usize, default: f64) -> f64 {
    args.get(index).map(|v| v.to_number()).unwrap_or(default)
}

fn js_char_index(index: f64, len: usize) -> Option<usize> {
    let len_i = len as i64;
    let mut idx = if index.is_nan() {
        0
    } else {
        index.trunc() as i64
    };
    if idx < 0 {
        idx += len_i;
    }
    if idx < 0 || idx >= len_i {
        None
    } else {
        Some(idx as usize)
    }
}

fn normalize_slice_index(index: f64, len: usize) -> usize {
    let len_i = len as i64;
    let mut idx = if index.is_nan() {
        0
    } else {
        index.trunc() as i64
    };
    if idx < 0 {
        idx += len_i;
    }
    idx.clamp(0, len_i) as usize
}

pub fn install_string(vm: &mut Vm) {
    vm.register_host_fn("String.prototype.toUpperCase", |vm, _args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        Ok(vm.alloc_string(s.to_uppercase()))
    });

    vm.register_host_fn("String.prototype.toLowerCase", |vm, _args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        Ok(vm.alloc_string(s.to_lowercase()))
    });

    vm.register_host_fn("String.prototype.trim", |vm, _args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        Ok(vm.alloc_string(s.trim().to_string()))
    });

    vm.register_host_fn("String.prototype.trimStart", |vm, _args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        Ok(vm.alloc_string(s.trim_start().to_string()))
    });

    vm.register_host_fn("String.prototype.trimEnd", |vm, _args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        Ok(vm.alloc_string(s.trim_end().to_string()))
    });

    vm.register_host_fn("String.prototype.charAt", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let index = arg_number(args, 0, 0.0);
        let result = js_char_index(index, s.len())
            .and_then(|i| s.chars().nth(i).map(|c| c.to_string()))
            .unwrap_or_default();
        Ok(vm.alloc_string(result))
    });

    vm.register_host_fn("String.prototype.charCodeAt", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let index = arg_number(args, 0, 0.0);
        let code = js_char_index(index, s.len())
            .and_then(|i| s.chars().nth(i).map(|c| c as u32))
            .map(f64::from)
            .unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(code))
    });

    vm.register_host_fn("String.prototype.indexOf", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let search = arg_string(vm, args, 0);
        let from = arg_number(args, 1, 0.0).max(0.0) as usize;
        let haystack = if from >= s.len() { "" } else { &s[from..] };
        let idx = haystack.find(&search).map(|i| i + from).unwrap_or(usize::MAX);
        let result = if idx == usize::MAX {
            -1
        } else {
            idx as i32
        };
        Ok(JsValue::from_i32(result))
    });

    vm.register_host_fn("String.prototype.lastIndexOf", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let search = arg_string(vm, args, 0);
        let from = arg_number(args, 1, f64::NAN);
        let end = if from.is_nan() {
            s.len()
        } else {
            normalize_slice_index(from, s.len()) + search.len()
        }
        .min(s.len());
        let haystack = &s[..end];
        let idx = haystack.rfind(&search);
        Ok(JsValue::from_i32(idx.map(|i| i as i32).unwrap_or(-1)))
    });

    vm.register_host_fn("String.prototype.includes", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let search = arg_string(vm, args, 0);
        let from = arg_number(args, 1, 0.0).max(0.0) as usize;
        let haystack = if from >= s.len() { "" } else { &s[from..] };
        Ok(JsValue::from_bool(haystack.contains(&search)))
    });

    vm.register_host_fn("String.prototype.startsWith", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let prefix = arg_string(vm, args, 0);
        let from = arg_number(args, 1, 0.0).max(0.0) as usize;
        let haystack = if from >= s.len() { "" } else { &s[from..] };
        Ok(JsValue::from_bool(haystack.starts_with(&prefix)))
    });

    vm.register_host_fn("String.prototype.endsWith", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let suffix = arg_string(vm, args, 0);
        let end = arg_number(args, 1, f64::NAN);
        let end = if end.is_nan() {
            s.len()
        } else {
            normalize_slice_index(end, s.len())
        };
        let haystack = &s[..end.min(s.len())];
        Ok(JsValue::from_bool(haystack.ends_with(&suffix)))
    });

    vm.register_host_fn("String.prototype.slice", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let len = s.len();
        let start = normalize_slice_index(arg_number(args, 0, 0.0), len);
        let end = if args.len() > 1 {
            normalize_slice_index(arg_number(args, 1, len as f64), len)
        } else {
            len
        };
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (start, start)
        };
        Ok(vm.alloc_string(s[start..end].to_string()))
    });

    vm.register_host_fn("String.prototype.substring", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let len = s.len();
        let mut start = normalize_slice_index(arg_number(args, 0, 0.0), len);
        let mut end = if args.len() > 1 {
            normalize_slice_index(arg_number(args, 1, len as f64), len)
        } else {
            len
        };
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }
        Ok(vm.alloc_string(s[start..end].to_string()))
    });

    vm.register_host_fn("String.prototype.replace", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let search = arg_string(vm, args, 0);
        let replacement = arg_string(vm, args, 1);
        let result = if let Some(pos) = s.find(&search) {
            let mut out = String::with_capacity(s.len() - search.len() + replacement.len());
            out.push_str(&s[..pos]);
            out.push_str(&replacement);
            out.push_str(&s[pos + search.len()..]);
            out
        } else {
            s
        };
        Ok(vm.alloc_string(result))
    });

    vm.register_host_fn("String.prototype.replaceAll", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let search = arg_string(vm, args, 0);
        let replacement = arg_string(vm, args, 1);
        Ok(vm.alloc_string(s.replace(&search, &replacement)))
    });

    vm.register_host_fn("String.prototype.split", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let separator = args.first().copied().map(|v| vm.value_to_string(v));
        let limit = args.get(1).map(|v| v.to_number().max(0.0) as usize);
        let sep = separator.as_deref();
        if sep == Some("") {
            let chars: Vec<String> = s.chars().map(|c| c.to_string()).collect();
            let take = limit.unwrap_or(chars.len()).min(chars.len());
            let values: Vec<JsValue> = chars
                .into_iter()
                .take(take)
                .map(|ch| vm.alloc_string(ch))
                .collect();
            let arr = vm.new_array(values.len() as u32);
            if let Some(obj) = vm.get_object_mut(arr) {
                for (i, val) in values.into_iter().enumerate() {
                    obj.set_property(i.to_string(), val);
                }
            }
            Ok(arr)
        } else {
            let parts: Vec<String> = match (sep, limit) {
                (None, _) => vec![s],
                (Some(sep), Some(limit)) => s.splitn(limit + 1, sep)
                    .map(str::to_string)
                    .collect(),
                (Some(sep), None) => s.split(sep).map(str::to_string).collect(),
            };
            let values: Vec<JsValue> = parts
                .into_iter()
                .map(|part| vm.alloc_string(part))
                .collect();
            let arr = vm.new_array(values.len() as u32);
            if let Some(obj) = vm.get_object_mut(arr) {
                for (i, val) in values.into_iter().enumerate() {
                    obj.set_property(i.to_string(), val);
                }
            }
            Ok(arr)
        }
    });

    vm.register_host_fn("String.prototype.repeat", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let count = arg_number(args, 0, f64::NAN);
        if count.is_nan() || count.is_infinite() || count < 0.0 {
            return Ok(JsValue::from_f64(f64::NAN));
        }
        let count = count.trunc() as usize;
        Ok(vm.alloc_string(s.repeat(count)))
    });

    vm.register_host_fn("String.prototype.padStart", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let target = arg_number(args, 0, 0.0).trunc() as usize;
        let pad = if args.len() > 1 {
            arg_string(vm, args, 1)
        } else {
            " ".to_string()
        };
        let pad = if pad.is_empty() { " ".to_string() } else { pad };
        if s.len() >= target {
            return Ok(vm.alloc_string(s));
        }
        let pad_len = target - s.len();
        let mut fill = String::new();
        while fill.len() < pad_len {
            fill.push_str(&pad);
        }
        fill.truncate(pad_len);
        Ok(vm.alloc_string(format!("{fill}{s}")))
    });

    vm.register_host_fn("String.prototype.padEnd", |vm, args| {
        let this = vm.get_global("this");
        let s = vm.value_to_string(this);
        let target = arg_number(args, 0, 0.0).trunc() as usize;
        let pad = if args.len() > 1 {
            arg_string(vm, args, 1)
        } else {
            " ".to_string()
        };
        let pad = if pad.is_empty() { " ".to_string() } else { pad };
        if s.len() >= target {
            return Ok(vm.alloc_string(s));
        }
        let pad_len = target - s.len();
        let mut fill = String::new();
        while fill.len() < pad_len {
            fill.push_str(&pad);
        }
        fill.truncate(pad_len);
        Ok(vm.alloc_string(format!("{s}{fill}")))
    });

    vm.register_host_fn("String.prototype.concat", |vm, args| {
        let this = vm.get_global("this");
        let mut result = vm.value_to_string(this);
        for &arg in args {
            result.push_str(&vm.value_to_string(arg));
        }
        Ok(vm.alloc_string(result))
    });
}
