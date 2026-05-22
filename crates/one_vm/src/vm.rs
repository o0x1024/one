use std::collections::HashMap;

use one_compiler::{CodeBlock, Constant, Opcode};
use one_core::{JsValue, OneError, OneResult};
use one_gc::Heap;

use crate::object::{JsObject, ObjectKind};

const HOST_SENTINEL_MASK: u64 = 0xDEAD_0000;

struct CallFrame {
    code: *const CodeBlock,
    pc: usize,
    base: usize,
}

/// Native function callable from JS
pub type HostFunction = Box<dyn Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue>>;

pub struct Vm {
    stack: Vec<JsValue>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, JsValue>,
    heap: Heap,
    string_table: Vec<String>,
    host_functions: Vec<(String, HostFunction)>,
}

impl Vm {
    pub fn new() -> Self {
        Vm {
            stack: Vec::with_capacity(1024),
            frames: Vec::new(),
            globals: HashMap::new(),
            heap: Heap::new(),
            string_table: Vec::new(),
            host_functions: Vec::new(),
        }
    }

    /// Register a native host function
    pub fn register_host_fn<F>(&mut self, name: &str, func: F)
    where
        F: Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue> + 'static,
    {
        let name = name.to_string();

        let fn_idx = self.host_functions.len();
        self.host_functions
            .push((name.clone(), Box::new(func)));
        let sentinel = JsValue::from_object_raw(HOST_SENTINEL_MASK | fn_idx as u64);

        if let Some((parent_name, method_name)) = name.rsplit_once('.') {
            let parent_val = self.globals.get(parent_name).copied();
            if let Some(parent_val) = parent_val {
                if let Some(obj) = self.get_object_mut(parent_val) {
                    obj.set_property(method_name.to_string(), sentinel);
                }
            } else {
                let mut ns_obj = JsObject::with_kind(ObjectKind::HostObject {
                    name: parent_name.to_string(),
                });
                ns_obj.set_property(method_name.to_string(), sentinel);
                let ns_val = self.alloc_object(ns_obj);
                self.globals.insert(parent_name.to_string(), ns_val);
            }
        } else {
            self.globals.insert(name, sentinel);
        }
    }

    fn host_sentinel_idx(val: JsValue) -> Option<usize> {
        if val.is_object() {
            let raw = val.as_object_raw()?;
            if raw & 0xFFFF_0000 == HOST_SENTINEL_MASK {
                return Some((raw & 0xFFFF) as usize);
            }
        }
        None
    }

    /// Allocate a JsObject on the heap and return a JsValue pointing to it
    pub fn alloc_object(&mut self, obj: JsObject) -> JsValue {
        let ptr = self.heap.alloc(obj);
        JsValue::from_object_raw(ptr as u64)
    }

    /// Get a reference to a JsObject from a JsValue
    pub fn get_object(&self, val: JsValue) -> Option<&JsObject> {
        if val.is_object() {
            let raw = val.as_object_raw()?;
            if raw & 0xFFFF_0000 == HOST_SENTINEL_MASK {
                return None;
            }
            Some(unsafe { &*(raw as *const JsObject) })
        } else {
            None
        }
    }

    /// Get a mutable reference to a JsObject from a JsValue
    pub fn get_object_mut(&mut self, val: JsValue) -> Option<&mut JsObject> {
        if val.is_object() {
            let raw = val.as_object_raw()?;
            if raw & 0xFFFF_0000 == HOST_SENTINEL_MASK {
                return None;
            }
            Some(unsafe { &mut *(raw as *mut JsObject) })
        } else {
            None
        }
    }

    /// Execute a CodeBlock
    pub fn execute(&mut self, code: &CodeBlock) -> OneResult<JsValue> {
        let base = self.stack.len();
        self.stack
            .resize(base + code.register_count as usize, JsValue::undefined());

        self.frames.push(CallFrame {
            code: code as *const CodeBlock,
            pc: 0,
            base,
        });

        self.run()
    }

    fn run(&mut self) -> OneResult<JsValue> {
        loop {
            let frame_idx = self.frames.len() - 1;
            let code_ptr = self.frames[frame_idx].code;
            let code = unsafe { &*code_ptr };

            if self.frames[frame_idx].pc >= code.bytecode.len() {
                return Ok(JsValue::undefined());
            }

            let instr = code.bytecode[self.frames[frame_idx].pc];
            self.frames[frame_idx].pc += 1;
            let base = self.frames[frame_idx].base;

            match instr.opcode() {
                Opcode::LoadConst => {
                    let dest = instr.a();
                    let idx = instr.bx() as usize;
                    let value = self.constant_to_value(&code.constants[idx]);
                    self.stack[base + dest as usize] = value;
                }
                Opcode::LoadInt => {
                    let dest = instr.a();
                    let val = instr.sbx() as i32;
                    self.stack[base + dest as usize] = JsValue::from_i32(val);
                }
                Opcode::LoadTrue => {
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(true);
                }
                Opcode::LoadFalse => {
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(false);
                }
                Opcode::LoadNull => {
                    self.stack[base + instr.a() as usize] = JsValue::null();
                }
                Opcode::LoadUndef => {
                    self.stack[base + instr.a() as usize] = JsValue::undefined();
                }
                Opcode::Move => {
                    let val = self.stack[base + instr.b() as usize];
                    self.stack[base + instr.a() as usize] = val;
                }
                Opcode::Add => {
                    let b = self.stack[base + instr.b() as usize];
                    let c = self.stack[base + instr.c() as usize];
                    if b.is_string() || c.is_string() {
                        let sb = self.value_to_string(b);
                        let sc = self.value_to_string(c);
                        let result = format!("{sb}{sc}");
                        let val = self.alloc_string(result);
                        self.stack[base + instr.a() as usize] = val;
                    } else {
                        let result = b.to_number() + c.to_number();
                        self.stack[base + instr.a() as usize] = JsValue::from_f64(result);
                    }
                }
                Opcode::Sub => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_f64(b - c);
                }
                Opcode::Mul => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_f64(b * c);
                }
                Opcode::Div => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_f64(b / c);
                }
                Opcode::Mod => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_f64(b % c);
                }
                Opcode::Neg => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_f64(-b);
                }
                Opcode::Lt => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(b < c);
                }
                Opcode::LtEq => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(b <= c);
                }
                Opcode::Gt => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(b > c);
                }
                Opcode::GtEq => {
                    let b = self.stack[base + instr.b() as usize].to_number();
                    let c = self.stack[base + instr.c() as usize].to_number();
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(b >= c);
                }
                Opcode::StrictEq => {
                    let b = self.stack[base + instr.b() as usize];
                    let c = self.stack[base + instr.c() as usize];
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(b == c);
                }
                Opcode::Not => {
                    let b = self.stack[base + instr.b() as usize];
                    let truthy = self.is_truthy(b);
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(!truthy);
                }
                Opcode::Jump => {
                    let offset = instr.sbx() as i32;
                    self.frames[frame_idx].pc =
                        (self.frames[frame_idx].pc as i32 + offset) as usize;
                }
                Opcode::JumpIfTrue => {
                    let val = self.stack[base + instr.a() as usize];
                    if self.is_truthy(val) {
                        let offset = instr.sbx() as i32;
                        self.frames[frame_idx].pc =
                            (self.frames[frame_idx].pc as i32 + offset) as usize;
                    }
                }
                Opcode::JumpIfFalse => {
                    let val = self.stack[base + instr.a() as usize];
                    if !self.is_truthy(val) {
                        let offset = instr.sbx() as i32;
                        self.frames[frame_idx].pc =
                            (self.frames[frame_idx].pc as i32 + offset) as usize;
                    }
                }
                Opcode::GetGlobal => {
                    let dest = instr.a();
                    let name_idx = instr.bx() as usize;
                    let name = match &code.constants[name_idx] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(OneError::InternalError(
                                "GetGlobal: expected string constant".into(),
                            ));
                        }
                    };
                    let value = self
                        .globals
                        .get(&name)
                        .copied()
                        .unwrap_or(JsValue::undefined());
                    self.stack[base + dest as usize] = value;
                }
                Opcode::SetGlobal => {
                    let src = instr.a();
                    let name_idx = instr.bx() as usize;
                    let name = match &code.constants[name_idx] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(OneError::InternalError(
                                "SetGlobal: expected string constant".into(),
                            ));
                        }
                    };
                    let value = self.stack[base + src as usize];
                    self.globals.insert(name, value);
                }
                Opcode::GetProp => {
                    let dest = instr.a();
                    let obj_val = self.stack[base + instr.b() as usize];
                    let name_idx = instr.c() as usize;
                    let name = match &code.constants[name_idx] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(OneError::InternalError(
                                "GetProp: expected string constant".into(),
                            ));
                        }
                    };

                    if let Some(obj) = self.get_object(obj_val) {
                        let value = obj.get_property(&name).unwrap_or(JsValue::undefined());
                        self.stack[base + dest as usize] = value;
                    } else {
                        self.stack[base + dest as usize] = JsValue::undefined();
                    }
                }
                Opcode::SetProp => {
                    let obj_val = self.stack[base + instr.a() as usize];
                    let name_idx = instr.b() as usize;
                    let value = self.stack[base + instr.c() as usize];
                    let name = match &code.constants[name_idx] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(OneError::InternalError(
                                "SetProp: expected string constant".into(),
                            ));
                        }
                    };
                    if let Some(obj) = self.get_object_mut(obj_val) {
                        obj.set_property(name, value);
                    }
                }
                Opcode::CreateObject => {
                    let dest = instr.a();
                    let obj = JsObject::new();
                    let val = self.alloc_object(obj);
                    self.stack[base + dest as usize] = val;
                }
                Opcode::InitProp => {
                    let obj_val = self.stack[base + instr.a() as usize];
                    let name_idx = instr.b() as usize;
                    let value = self.stack[base + instr.c() as usize];
                    let name = match &code.constants[name_idx] {
                        Constant::String(s) => s.clone(),
                        _ => {
                            return Err(OneError::InternalError(
                                "InitProp: expected string constant".into(),
                            ));
                        }
                    };
                    if let Some(obj) = self.get_object_mut(obj_val) {
                        obj.set_property(name, value);
                    }
                }
                Opcode::CreateArray => {
                    let dest = instr.a();
                    let len = instr.b();
                    let obj = JsObject::with_kind(ObjectKind::Array {
                        length: len as u32,
                    });
                    let val = self.alloc_object(obj);
                    self.stack[base + dest as usize] = val;
                }
                Opcode::SetArrayElem => {
                    let obj_val = self.stack[base + instr.a() as usize];
                    let index = instr.b();
                    let value = self.stack[base + instr.c() as usize];
                    if let Some(obj) = self.get_object_mut(obj_val) {
                        obj.set_property(index.to_string(), value);
                        if let ObjectKind::Array { length } = obj.kind_mut()
                            && index as u32 >= *length
                        {
                            *length = index as u32 + 1;
                        }
                    }
                }
                Opcode::Call => {
                    let dest = instr.a();
                    let func_reg = instr.b();
                    let argc = instr.c() as usize;
                    let func_val = self.stack[base + func_reg as usize];

                    if let Some(idx) = Self::host_sentinel_idx(func_val) {
                        if idx >= self.host_functions.len() {
                            return Err(OneError::InternalError(format!(
                                "unknown host fn index: {idx}"
                            )));
                        }

                        let args: Vec<JsValue> = (0..argc)
                            .map(|i| self.stack[base + func_reg as usize + 1 + i])
                            .collect();

                        let placeholder: HostFunction =
                            Box::new(|_, _| Ok(JsValue::undefined()));
                        let host_fn =
                            std::mem::replace(&mut self.host_functions[idx].1, placeholder);
                        let result = host_fn(self, &args)?;
                        self.host_functions[idx].1 = host_fn;

                        self.stack[base + dest as usize] = result;
                        continue;
                    }

                    self.stack[base + dest as usize] = JsValue::undefined();
                }
                Opcode::Return => {
                    let val = self.stack[base + instr.a() as usize];
                    self.frames.pop();
                    self.stack.truncate(base);
                    if self.frames.is_empty() {
                        return Ok(val);
                    }
                    return Ok(val);
                }
                Opcode::ReturnUndef => {
                    self.frames.pop();
                    self.stack.truncate(base);
                    return Ok(JsValue::undefined());
                }
                _ => {}
            }
        }
    }

    fn constant_to_value(&mut self, constant: &Constant) -> JsValue {
        match constant {
            Constant::Number(n) => JsValue::from_f64(*n),
            Constant::Integer(i) => JsValue::from_i32(*i),
            Constant::String(s) => self.alloc_string(s.clone()),
            Constant::Boolean(b) => JsValue::from_bool(*b),
            Constant::Null => JsValue::null(),
            Constant::Undefined => JsValue::undefined(),
        }
    }

    fn alloc_string(&mut self, s: String) -> JsValue {
        let idx = self.string_table.len();
        self.string_table.push(s);
        JsValue::from_string_raw(idx as u64)
    }

    pub fn value_to_string(&self, val: JsValue) -> String {
        if val.is_string() {
            let idx = val.as_string_raw().unwrap() as usize;
            if idx < self.string_table.len() {
                return self.string_table[idx].clone();
            }
        }
        format!("{val}")
    }

    fn is_truthy(&self, val: JsValue) -> bool {
        if val.is_null() || val.is_undefined() {
            return false;
        }
        if val.is_boolean() {
            return val.as_bool().unwrap();
        }
        if val.is_int32() {
            return val.as_i32().unwrap() != 0;
        }
        if val.is_float64() {
            let n = val.as_f64().unwrap();
            return n != 0.0 && !n.is_nan();
        }
        if val.is_string() {
            let s = self.value_to_string(val);
            return !s.is_empty();
        }
        true
    }

    pub fn get_global(&self, name: &str) -> JsValue {
        self.globals
            .get(name)
            .copied()
            .unwrap_or(JsValue::undefined())
    }

    pub fn set_global(&mut self, name: &str, val: JsValue) {
        self.globals.insert(name.to_string(), val);
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
