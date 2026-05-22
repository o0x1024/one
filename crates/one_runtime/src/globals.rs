use one_compiler::Compiler;
use one_core::{CompileError, JsValue, OneError};
use one_parser::parser::Parser;
use one_vm::Vm;

use crate::number::{parse_float_string, parse_int_string};

fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric()
            || matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')')
        {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

fn decode_uri_component(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn install_globals(vm: &mut Vm) {
    vm.register_host_fn("eval", |vm, args| {
        let arg = args.first().copied().unwrap_or(JsValue::undefined());
        if !arg.is_string() {
            return Ok(arg);
        }

        let mut code_str = vm.value_to_string(arg);
        let trimmed = code_str.trim_end();
        if !trimmed.is_empty() && !trimmed.ends_with(';') && !trimmed.ends_with('}') {
            code_str = format!("{trimmed};");
        }

        let program = Parser::parse(&code_str).map_err(|e| {
            OneError::CompileError(CompileError {
                message: e.message,
                file: Some("<eval>".into()),
                line: 0,
                column: 0,
            })
        })?;

        let code = Compiler::compile_eval(&program);
        vm.execute_inner(&code)
    });

    vm.register_host_fn("parseInt", |vm, args| {
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

    vm.register_host_fn("parseFloat", |vm, args| {
        let s = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        Ok(JsValue::from_f64(parse_float_string(&s)))
    });

    vm.register_host_fn("isNaN", |_vm, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(JsValue::from_bool(val.to_number().is_nan()))
    });

    vm.register_host_fn("isFinite", |_vm, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(JsValue::from_bool(val.to_number().is_finite()))
    });

    vm.register_host_fn("encodeURIComponent", |vm, args| {
        let s = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        Ok(vm.alloc_string(encode_uri_component(&s)))
    });

    vm.register_host_fn("decodeURIComponent", |vm, args| {
        let s = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        Ok(vm.alloc_string(decode_uri_component(&s)))
    });
}
