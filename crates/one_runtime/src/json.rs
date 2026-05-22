use one_core::{JsValue, OneError, OneResult};
use one_vm::Vm;
use one_vm::object::{JsObject, ObjectKind};

fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn number_to_json(n: f64) -> String {
    if n.is_nan() || n.is_infinite() {
        return "null".to_string();
    }
    format!("{n}")
}

fn js_stringify(vm: &Vm, val: JsValue) -> String {
    if val.is_null() {
        return "null".to_string();
    }
    if val.is_undefined() {
        return "undefined".to_string();
    }
    if val.is_boolean() {
        return val.as_bool().unwrap().to_string();
    }
    if val.is_int32() {
        return val.as_i32().unwrap().to_string();
    }
    if val.is_float64() {
        return number_to_json(val.as_f64().unwrap());
    }
    if val.is_string() {
        let s = vm.value_to_string(val);
        return format!("\"{}\"", escape_json_string(&s));
    }
    if let Some(obj) = vm.get_object(val) {
        match obj.kind() {
            ObjectKind::Array { length } => {
                let mut parts = Vec::new();
                for i in 0..*length {
                    let elem = obj
                        .get_property(&i.to_string())
                        .unwrap_or(JsValue::null());
                    parts.push(js_stringify(vm, elem));
                }
                format!("[{}]", parts.join(","))
            }
            _ => {
                let mut parts = Vec::new();
                for key in obj.enumerable_keys() {
                    if let Some(value) = obj.get_property(&key) {
                        let val_str = js_stringify(vm, value);
                        parts.push(format!(
                            "\"{}\":{}",
                            escape_json_string(&key),
                            val_str
                        ));
                    }
                }
                format!("{{{}}}", parts.join(","))
            }
        }
    } else {
        "null".to_string()
    }
}

struct JsonParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.next();
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, expected: char) -> OneResult<()> {
        match self.next() {
            Some(ch) if ch == expected => Ok(()),
            _ => Err(OneError::js_exception(
                "SyntaxError",
                "Unexpected token in JSON",
            )),
        }
    }

    fn parse_value(&mut self, vm: &mut Vm) -> OneResult<JsValue> {
        self.skip_ws();
        match self.peek() {
            Some('"') => self.parse_string(vm),
            Some('[') => self.parse_array(vm),
            Some('{') => self.parse_object(vm),
            Some('t') => self.parse_literal("true", JsValue::from_bool(true)),
            Some('f') => self.parse_literal("false", JsValue::from_bool(false)),
            Some('n') => self.parse_literal("null", JsValue::null()),
            Some('-') | Some('0'..='9') => self.parse_number(),
            _ => Err(OneError::js_exception(
                "SyntaxError",
                "Unexpected token in JSON",
            )),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: JsValue) -> OneResult<JsValue> {
        if self.input[self.pos..].starts_with(literal) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(OneError::js_exception(
                "SyntaxError",
                "Unexpected token in JSON",
            ))
        }
    }

    fn parse_number(&mut self) -> OneResult<JsValue> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.next();
        }
        while matches!(self.peek(), Some('0'..='9')) {
            self.next();
        }
        if self.peek() == Some('.') {
            self.next();
            while matches!(self.peek(), Some('0'..='9')) {
                self.next();
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.next();
            if matches!(self.peek(), Some('+' | '-')) {
                self.next();
            }
            while matches!(self.peek(), Some('0'..='9')) {
                self.next();
            }
        }
        let slice = &self.input[start..self.pos];
        slice
            .parse::<f64>()
            .map(JsValue::from_f64)
            .map_err(|_| OneError::js_exception("SyntaxError", "Invalid number in JSON"))
    }

    fn parse_string(&mut self, vm: &mut Vm) -> OneResult<JsValue> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.next() {
                None => {
                    return Err(OneError::js_exception(
                        "SyntaxError",
                        "Unterminated string in JSON",
                    ));
                }
                Some('"') => break,
                Some('\\') => {
                    let esc = self.next().ok_or_else(|| {
                        OneError::js_exception("SyntaxError", "Invalid escape in JSON")
                    })?;
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\x08'),
                        'f' => out.push('\x0c'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hex: String = (0..4)
                                .filter_map(|_| self.next())
                                .collect();
                            if hex.len() != 4 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                                return Err(OneError::js_exception(
                                    "SyntaxError",
                                    "Invalid unicode escape in JSON",
                                ));
                            }
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| {
                                OneError::js_exception(
                                    "SyntaxError",
                                    "Invalid unicode escape in JSON",
                                )
                            })?;
                            let ch = char::from_u32(code).ok_or_else(|| {
                                OneError::js_exception(
                                    "SyntaxError",
                                    "Invalid unicode escape in JSON",
                                )
                            })?;
                            out.push(ch);
                        }
                        _ => {
                            return Err(OneError::js_exception(
                                "SyntaxError",
                                "Invalid escape in JSON",
                            ));
                        }
                    }
                }
                Some(ch) => out.push(ch),
            }
        }
        Ok(vm.alloc_string(out))
    }

    fn parse_array(&mut self, vm: &mut Vm) -> OneResult<JsValue> {
        self.expect('[')?;
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() != Some(']') {
            loop {
                items.push(self.parse_value(vm)?);
                self.skip_ws();
                match self.next() {
                    Some(',') => {
                        self.skip_ws();
                    }
                    Some(']') => break,
                    _ => {
                        return Err(OneError::js_exception(
                            "SyntaxError",
                            "Expected ',' or ']' in JSON array",
                        ));
                    }
                }
            }
        } else {
            self.next();
        }

        let len = items.len() as u32;
        let arr_val = vm.new_array(len);
        if let Some(arr_obj) = vm.get_object_mut(arr_val) {
            for (i, item) in items.into_iter().enumerate() {
                arr_obj.set_property(i.to_string(), item);
            }
        }
        Ok(arr_val)
    }

    fn parse_object(&mut self, vm: &mut Vm) -> OneResult<JsValue> {
        self.expect('{')?;
        self.skip_ws();
        let mut obj = JsObject::new();
        if self.peek() != Some('}') {
            loop {
                self.skip_ws();
                let key_val = self.parse_string(vm)?;
                let key = vm.value_to_string(key_val);
                self.skip_ws();
                self.expect(':')?;
                let value = self.parse_value(vm)?;
                obj.set_property(key, value);
                self.skip_ws();
                match self.next() {
                    Some(',') => {
                        self.skip_ws();
                    }
                    Some('}') => break,
                    _ => {
                        return Err(OneError::js_exception(
                            "SyntaxError",
                            "Expected ',' or '}' in JSON object",
                        ));
                    }
                }
            }
        } else {
            self.next();
        }
        Ok(vm.alloc_object(obj))
    }

    fn finish(mut self) -> OneResult<()> {
        self.skip_ws();
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(OneError::js_exception(
                "SyntaxError",
                "Unexpected trailing content in JSON",
            ))
        }
    }
}

fn json_parse(vm: &mut Vm, input: &str) -> OneResult<JsValue> {
    let trimmed = input.trim();
    let mut parser = JsonParser::new(trimmed);
    let value = parser.parse_value(vm)?;
    parser.finish()?;
    Ok(value)
}

pub fn install_json(vm: &mut Vm) {
    vm.register_host_fn("JSON.stringify", |vm, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        Ok(vm.alloc_string(js_stringify(vm, val)))
    });

    vm.register_host_fn("JSON.parse", |vm, args| {
        let text = args
            .first()
            .map(|v| vm.value_to_string(*v))
            .unwrap_or_default();
        json_parse(vm, &text)
    });
}
