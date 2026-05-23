use one_core::{JsValue, OneResult};
use one_vm::Vm;

pub fn install_fetch(vm: &mut Vm) {
    vm.register_host_fn("fetch", native_fetch);
}

fn native_fetch(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let url = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();

    let opts = args.get(1).copied();

    let method = extract_string_prop(vm, opts, "method")
        .unwrap_or_else(|| "GET".into())
        .to_uppercase();
    let body = extract_string_prop(vm, opts, "body");
    let headers = extract_headers(vm, opts);

    let has_body = method == "POST" || method == "PUT" || method == "PATCH";

    let result = if has_body {
        let mut req = match method.as_str() {
            "POST" => ureq::post(&url),
            "PUT" => ureq::put(&url),
            _ => ureq::patch(&url),
        };
        for (key, value) in &headers {
            req = req.header(key, value);
        }
        if let Some(body_str) = body {
            req.header("content-type", "application/json")
                .send(body_str.as_bytes())
        } else {
            req.send(&[] as &[u8])
        }
    } else {
        let mut req = match method.as_str() {
            "DELETE" => ureq::delete(&url),
            "HEAD" => ureq::head(&url),
            _ => ureq::get(&url),
        };
        for (key, value) in &headers {
            req = req.header(key, value);
        }
        req.call()
    };

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let ok = (200..300).contains(&status);
            let body_str = response.into_body().read_to_string().unwrap_or_default();

            let ok_val = JsValue::from_bool(ok);
            let status_val = JsValue::from_i32(status as i32);
            let status_text_val = vm.alloc_string(format!("{status}"));
            let body_val = vm.alloc_string(body_str);
            let headers_obj = vm.create_object_from_pairs(&[]);

            let result = vm.create_object_from_pairs(&[
                ("ok".into(), ok_val),
                ("status".into(), status_val),
                ("statusText".into(), status_text_val),
                ("body".into(), body_val),
                ("headers".into(), headers_obj),
            ]);
            Ok(result)
        }
        Err(ureq::Error::StatusCode(code)) => {
            let status = code as i32;
            let status_text = vm.alloc_string(format!("HTTP {code}"));
            let body_val = vm.alloc_string(String::new());
            let headers_obj = vm.create_object_from_pairs(&[]);
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("status".into(), JsValue::from_i32(status)),
                ("statusText".into(), status_text),
                ("body".into(), body_val),
                ("headers".into(), headers_obj),
            ]);
            Ok(result)
        }
        Err(e) => {
            let err_msg = e.to_string();
            let status_text = vm.alloc_string(err_msg);
            let body_val = vm.alloc_string(String::new());
            let headers_obj = vm.create_object_from_pairs(&[]);
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("status".into(), JsValue::from_i32(0)),
                ("statusText".into(), status_text),
                ("body".into(), body_val),
                ("headers".into(), headers_obj),
            ]);
            Ok(result)
        }
    }
}

fn extract_string_prop(vm: &Vm, obj: Option<JsValue>, key: &str) -> Option<String> {
    let obj_val = obj?;
    let obj_ref = vm.get_object(obj_val)?;
    let prop = obj_ref.get_property(key)?;
    if prop.is_undefined() || prop.is_null() {
        return None;
    }
    Some(vm.value_to_string(prop))
}

fn extract_headers(vm: &Vm, obj: Option<JsValue>) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let Some(obj_val) = obj else { return result };
    let Some(obj_ref) = vm.get_object(obj_val) else {
        return result;
    };
    let Some(headers_val) = obj_ref.get_property("headers") else {
        return result;
    };
    if headers_val.is_undefined() || headers_val.is_null() {
        return result;
    }
    let Some(headers_obj) = vm.get_object(headers_val) else {
        return result;
    };
    let keys = headers_obj.property_keys();
    for key in keys {
        if let Some(val) = headers_obj.get_property(&key) {
            result.push((key.to_string(), vm.value_to_string(val)));
        }
    }
    result
}
