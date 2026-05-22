use std::collections::HashMap;

use one_compiler::{CodeBlock, Constant, Opcode};
use one_core::{JsValue, OneError, OneResult};
use one_gc::Heap;

use crate::object::{FunctionObject, JsObject, MapData, ObjectKind, PromiseState, SetData};

const HOST_SENTINEL_MASK: u64 = 0xDEAD_0000;
const PROMISE_METHOD_MASK: u64 = 0xBEEF_0000;
const PROMISE_RESOLVER_MASK: u64 = 0xCAFE_0000;

struct CallFrame {
    code: *const CodeBlock,
    pc: usize,
    base: usize,
    dest: u8,
    is_constructor: bool,
    this_val: JsValue,
}

struct ExceptionHandlerFrame {
    catch_pc: usize,
    frame_idx: usize,
}

/// Native function callable from JS
pub type HostFunction = Box<dyn Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue>>;

struct MicroTask {
    callback: JsValue,
    arg: JsValue,
}

#[derive(Clone)]
enum PromiseMethodKind {
    Then,
    Catch,
}

#[derive(Clone)]
enum PromiseResolverKind {
    Resolve,
    Reject,
}

pub struct Vm {
    stack: Vec<JsValue>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, JsValue>,
    heap: Heap,
    string_table: Vec<String>,
    host_functions: Vec<(String, HostFunction)>,
    exception_stack: Vec<ExceptionHandlerFrame>,
    current_exception: Option<JsValue>,
    microtasks: Vec<MicroTask>,
    promise_methods: Vec<(JsValue, PromiseMethodKind)>,
    promise_resolvers: Vec<(JsValue, PromiseResolverKind)>,
    array_prototype: Option<JsValue>,
    map_prototype: Option<JsValue>,
    set_prototype: Option<JsValue>,
    date_prototype: Option<JsValue>,
    regexp_prototype: Option<JsValue>,
    symbol_counter: u32,
    symbol_descriptions: Vec<Option<String>>,
    global_symbols: HashMap<String, u32>,
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
            exception_stack: Vec::new(),
            current_exception: None,
            microtasks: Vec::new(),
            promise_methods: Vec::new(),
            promise_resolvers: Vec::new(),
            array_prototype: None,
            map_prototype: None,
            set_prototype: None,
            date_prototype: None,
            regexp_prototype: None,
            symbol_counter: 0,
            symbol_descriptions: Vec::new(),
            global_symbols: HashMap::new(),
        }
    }

    /// Register a native host function
    pub fn register_host_fn<F>(&mut self, name: &str, func: F)
    where
        F: Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue> + 'static,
    {
        let sentinel = self.register_host_fn_returning_sentinel(name, func);

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
            self.globals.insert(name.to_string(), sentinel);
        }
    }

    pub fn register_host_fn_returning_sentinel<F>(&mut self, name: &str, func: F) -> JsValue
    where
        F: Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue> + 'static,
    {
        let fn_idx = self.host_functions.len();
        self.host_functions
            .push((name.to_string(), Box::new(func)));
        JsValue::from_object_raw(HOST_SENTINEL_MASK | fn_idx as u64)
    }

    pub fn set_array_prototype(&mut self, proto: JsValue) {
        self.array_prototype = Some(proto);
    }

    pub fn set_map_prototype(&mut self, proto: JsValue) {
        self.map_prototype = Some(proto);
    }

    pub fn set_set_prototype(&mut self, proto: JsValue) {
        self.set_prototype = Some(proto);
    }

    pub fn set_date_prototype(&mut self, proto: JsValue) {
        self.date_prototype = Some(proto);
    }

    pub fn set_regexp_prototype(&mut self, proto: JsValue) {
        self.regexp_prototype = Some(proto);
    }

    pub fn create_symbol(&mut self, description: Option<String>) -> u32 {
        let id = self.symbol_counter;
        self.symbol_counter += 1;
        self.symbol_descriptions.push(description);
        id
    }

    pub fn get_or_create_global_symbol(&mut self, key: &str) -> u32 {
        if let Some(&id) = self.global_symbols.get(key) {
            return id;
        }
        let id = self.create_symbol(Some(key.to_string()));
        self.global_symbols.insert(key.to_string(), id);
        id
    }

    fn apply_array_prototype(&self, obj: &mut JsObject) {
        if let Some(proto_val) = self.array_prototype
            && let Some(raw) = proto_val.as_object_raw()
        {
            obj.set_prototype(Some(raw as *mut JsObject));
        }
    }

    fn apply_map_prototype(&self, obj: &mut JsObject) {
        if let Some(proto_val) = self.map_prototype
            && let Some(raw) = proto_val.as_object_raw()
        {
            obj.set_prototype(Some(raw as *mut JsObject));
        }
    }

    fn apply_set_prototype(&self, obj: &mut JsObject) {
        if let Some(proto_val) = self.set_prototype
            && let Some(raw) = proto_val.as_object_raw()
        {
            obj.set_prototype(Some(raw as *mut JsObject));
        }
    }

    fn apply_date_prototype(&self, obj: &mut JsObject) {
        if let Some(proto_val) = self.date_prototype
            && let Some(raw) = proto_val.as_object_raw()
        {
            obj.set_prototype(Some(raw as *mut JsObject));
        }
    }

    fn apply_regexp_prototype(&self, obj: &mut JsObject) {
        if let Some(proto_val) = self.regexp_prototype
            && let Some(raw) = proto_val.as_object_raw()
        {
            obj.set_prototype(Some(raw as *mut JsObject));
        }
    }

    pub fn new_array(&mut self, length: u32) -> JsValue {
        let mut obj = JsObject::with_kind(ObjectKind::Array { length });
        self.apply_array_prototype(&mut obj);
        self.alloc_object(obj)
    }

    pub fn new_map(&mut self) -> JsValue {
        let mut obj = JsObject::with_kind(ObjectKind::Map(MapData { entries: Vec::new() }));
        self.apply_map_prototype(&mut obj);
        self.alloc_object(obj)
    }

    pub fn new_set(&mut self) -> JsValue {
        let mut obj = JsObject::with_kind(ObjectKind::Set(SetData { values: Vec::new() }));
        self.apply_set_prototype(&mut obj);
        self.alloc_object(obj)
    }

    pub fn new_date(&mut self, ms: f64) -> JsValue {
        let mut obj = JsObject::with_kind(ObjectKind::Date(ms));
        self.apply_date_prototype(&mut obj);
        self.alloc_object(obj)
    }

    pub fn new_regexp(&mut self, pattern: String, flags: String) -> JsValue {
        let mut obj = JsObject::with_kind(ObjectKind::RegExp { pattern, flags });
        self.apply_regexp_prototype(&mut obj);
        self.alloc_object(obj)
    }

    pub fn date_ms(&self, val: JsValue) -> Option<f64> {
        self.get_object(val).and_then(|obj| {
            if let ObjectKind::Date(ms) = obj.kind() {
                Some(*ms)
            } else {
                None
            }
        })
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

    fn promise_method_idx(val: JsValue) -> Option<usize> {
        if val.is_object() {
            let raw = val.as_object_raw()?;
            if raw & 0xFFFF_0000 == PROMISE_METHOD_MASK {
                return Some((raw & 0xFFFF) as usize);
            }
        }
        None
    }

    fn promise_method_sentinel(idx: usize) -> JsValue {
        JsValue::from_object_raw(PROMISE_METHOD_MASK | idx as u64)
    }

    fn promise_resolver_idx(val: JsValue) -> Option<usize> {
        if val.is_object() {
            let raw = val.as_object_raw()?;
            if raw & 0xFFFF_0000 == PROMISE_RESOLVER_MASK {
                return Some((raw & 0xFFFF) as usize);
            }
        }
        None
    }

    fn promise_resolver_sentinel(idx: usize) -> JsValue {
        JsValue::from_object_raw(PROMISE_RESOLVER_MASK | idx as u64)
    }

    fn find_host_fn(&self, name: &str) -> Option<usize> {
        self.host_functions
            .iter()
            .position(|(n, _)| n == name)
    }

    pub fn get_primitive_method(&self, type_name: &str, method_name: &str) -> Option<JsValue> {
        let full_name = format!("{type_name}.prototype.{method_name}");
        self.find_host_fn(&full_name).map(|idx| {
            JsValue::from_object_raw(HOST_SENTINEL_MASK | idx as u64)
        })
    }

    fn invoke_host_fn(&mut self, idx: usize, args: &[JsValue]) -> OneResult<JsValue> {
        if idx >= self.host_functions.len() {
            return Err(OneError::InternalError(format!(
                "unknown host fn index: {idx}"
            )));
        }

        let placeholder: HostFunction = Box::new(|_, _| Ok(JsValue::undefined()));
        let host_fn = std::mem::replace(&mut self.host_functions[idx].1, placeholder);
        let result = host_fn(self, args)?;
        self.host_functions[idx].1 = host_fn;
        Ok(result)
    }

    pub fn alloc_promise(&mut self, state: PromiseState) -> JsValue {
        let obj = JsObject::with_kind(ObjectKind::Promise(state));
        let promise_val = self.alloc_object(obj);

        let then_idx = self.promise_methods.len();
        self.promise_methods
            .push((promise_val, PromiseMethodKind::Then));
        let catch_idx = self.promise_methods.len();
        self.promise_methods
            .push((promise_val, PromiseMethodKind::Catch));

        if let Some(obj) = self.get_object_mut(promise_val) {
            obj.set_property(
                "then".to_string(),
                Self::promise_method_sentinel(then_idx),
            );
            obj.set_property(
                "catch".to_string(),
                Self::promise_method_sentinel(catch_idx),
            );
        }

        promise_val
    }

    pub fn create_promise_resolver(&mut self, promise_val: JsValue, resolve: bool) -> JsValue {
        let idx = self.promise_resolvers.len();
        let kind = if resolve {
            PromiseResolverKind::Resolve
        } else {
            PromiseResolverKind::Reject
        };
        self.promise_resolvers.push((promise_val, kind));
        Self::promise_resolver_sentinel(idx)
    }

    fn schedule_microtask(&mut self, callback: JsValue, arg: JsValue) {
        self.microtasks.push(MicroTask { callback, arg });
    }

    fn invoke_callback(&mut self, callback: JsValue, arg: JsValue) -> OneResult<()> {
        if callback.is_undefined() || callback.is_null() {
            return Ok(());
        }
        self.call_function(callback, &[arg])?;
        Ok(())
    }

    fn settle_promise_fulfilled(&mut self, promise_val: JsValue, value: JsValue) -> OneResult<()> {
        let pending = if let Some(obj) = self.get_object_mut(promise_val) {
            if let ObjectKind::Promise(PromiseState::Pending { .. }) = obj.kind() {
                if let ObjectKind::Promise(state) = obj.kind_mut() {
                    if let PromiseState::Pending {
                        on_fulfilled,
                        on_rejected: _,
                    } = std::mem::replace(state, PromiseState::Fulfilled(value))
                    {
                        Some(on_fulfilled)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(handlers) = pending {
            for callback in handlers {
                self.schedule_microtask(callback, value);
            }
        }
        Ok(())
    }

    fn settle_promise_rejected(&mut self, promise_val: JsValue, reason: JsValue) -> OneResult<()> {
        let pending = if let Some(obj) = self.get_object_mut(promise_val) {
            if let ObjectKind::Promise(PromiseState::Pending { .. }) = obj.kind() {
                if let ObjectKind::Promise(state) = obj.kind_mut() {
                    if let PromiseState::Pending {
                        on_fulfilled: _,
                        on_rejected,
                    } = std::mem::replace(state, PromiseState::Rejected(reason))
                    {
                        Some(on_rejected)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(handlers) = pending {
            for callback in handlers {
                self.schedule_microtask(callback, reason);
            }
        }
        Ok(())
    }

    fn promise_then(
        &mut self,
        promise_val: JsValue,
        on_fulfilled: JsValue,
        on_rejected: JsValue,
    ) -> OneResult<JsValue> {
        if let Some(obj) = self.get_object(promise_val) {
            match obj.kind() {
                ObjectKind::Promise(PromiseState::Fulfilled(value)) => {
                    let value = *value;
                    self.invoke_callback(on_fulfilled, value)?;
                }
                ObjectKind::Promise(PromiseState::Rejected(reason)) => {
                    let reason = *reason;
                    self.invoke_callback(on_rejected, reason)?;
                }
                ObjectKind::Promise(PromiseState::Pending { .. }) => {
                    if let Some(obj) = self.get_object_mut(promise_val)
                        && let ObjectKind::Promise(PromiseState::Pending {
                            on_fulfilled: fulfilled_handlers,
                            on_rejected: rejected_handlers,
                        }) = obj.kind_mut()
                    {
                        if !on_fulfilled.is_undefined() {
                            fulfilled_handlers.push(on_fulfilled);
                        }
                        if !on_rejected.is_undefined() {
                            rejected_handlers.push(on_rejected);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(promise_val)
    }

    fn promise_catch(&mut self, promise_val: JsValue, on_rejected: JsValue) -> OneResult<JsValue> {
        self.promise_then(promise_val, JsValue::undefined(), on_rejected)
    }

    fn drain_microtasks(&mut self) -> OneResult<()> {
        while !self.microtasks.is_empty() {
            let tasks: Vec<MicroTask> = std::mem::take(&mut self.microtasks);
            for task in tasks {
                self.call_function(task.callback, &[task.arg])?;
            }
        }
        Ok(())
    }

    pub fn call_function(&mut self, func_val: JsValue, args: &[JsValue]) -> OneResult<JsValue> {
        if let Some(idx) = Self::host_sentinel_idx(func_val) {
            return self.invoke_host_fn(idx, args);
        }

        if let Some(idx) = Self::promise_resolver_idx(func_val) {
            if idx < self.promise_resolvers.len() {
                let (promise_val, kind) = self.promise_resolvers[idx].clone();
                let arg = args.first().copied().unwrap_or(JsValue::undefined());
                match kind {
                    PromiseResolverKind::Resolve => {
                        self.settle_promise_fulfilled(promise_val, arg)?;
                    }
                    PromiseResolverKind::Reject => {
                        self.settle_promise_rejected(promise_val, arg)?;
                    }
                }
            }
            return Ok(JsValue::undefined());
        }

        if let Some(idx) = Self::promise_method_idx(func_val) {
            if idx < self.promise_methods.len() {
                let (promise_val, method) = &self.promise_methods[idx].clone();
                let on_fulfilled = args.first().copied().unwrap_or(JsValue::undefined());
                let on_rejected = args.get(1).copied().unwrap_or(JsValue::undefined());
                return match method {
                    PromiseMethodKind::Then => {
                        self.promise_then(*promise_val, on_fulfilled, on_rejected)
                    }
                    PromiseMethodKind::Catch => self.promise_catch(*promise_val, on_fulfilled),
                };
            }
            return Ok(JsValue::undefined());
        }

        if let Some(obj) = self.get_object(func_val)
            && let ObjectKind::Function(func_obj) = obj.kind()
        {
            let code = &func_obj.code as *const CodeBlock;
            let code_ref = unsafe { &*code };
            let base = self.stack.len();
            self.stack
                .resize(base + code_ref.register_count as usize, JsValue::undefined());
            for (i, &arg) in args.iter().enumerate() {
                if i < code_ref.param_count as usize {
                    self.stack[base + i] = arg;
                }
            }
            let caller_depth = self.frames.len();
            self.frames.push(CallFrame {
                code,
                pc: 0,
                base,
                dest: 0,
                is_constructor: false,
                this_val: JsValue::undefined(),
            });
            return self.run_until(Some(caller_depth));
        }
        Ok(JsValue::undefined())
    }

    /// Allocate a JsObject on the heap and return a JsValue pointing to it
    pub fn alloc_object(&mut self, obj: JsObject) -> JsValue {
        if self.heap.should_collect() {
            self.run_gc();
        }
        let ptr = self.heap.alloc(obj);
        JsValue::from_object_raw(ptr as u64)
    }

    fn run_gc(&mut self) {
        let mut roots = Vec::new();
        self.collect_gc_roots(&mut roots);
        self.heap.collect(&roots);
        self.heap.grow_threshold();
    }

    fn collect_gc_roots(&self, roots: &mut Vec<*const u8>) {
        for val in &self.stack {
            Self::push_object_root(roots, *val);
        }
        for val in self.globals.values() {
            Self::push_object_root(roots, *val);
        }
        for frame in &self.frames {
            Self::push_object_root(roots, frame.this_val);
        }
        for (promise_val, _) in &self.promise_methods {
            Self::push_object_root(roots, *promise_val);
        }
        for (promise_val, _) in &self.promise_resolvers {
            Self::push_object_root(roots, *promise_val);
        }
        if let Some(proto) = self.array_prototype {
            Self::push_object_root(roots, proto);
        }
    }

    fn push_object_root(roots: &mut Vec<*const u8>, val: JsValue) {
        if val.is_object()
            && let Some(raw) = val.as_object_raw()
            && raw & 0xFFFF_0000 != HOST_SENTINEL_MASK
            && raw & 0xFFFF_0000 != PROMISE_METHOD_MASK
            && raw & 0xFFFF_0000 != PROMISE_RESOLVER_MASK
        {
            roots.push(raw as *const u8);
        }
    }

    pub fn set_gc_threshold(&mut self, threshold: usize) {
        self.heap.set_gc_threshold(threshold);
    }

    /// Get a reference to a JsObject from a JsValue
    pub fn get_object(&self, val: JsValue) -> Option<&JsObject> {
        if val.is_object() {
            let raw = val.as_object_raw()?;
            if raw & 0xFFFF_0000 == HOST_SENTINEL_MASK
                || raw & 0xFFFF_0000 == PROMISE_METHOD_MASK
                || raw & 0xFFFF_0000 == PROMISE_RESOLVER_MASK
            {
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
            if raw & 0xFFFF_0000 == HOST_SENTINEL_MASK
                || raw & 0xFFFF_0000 == PROMISE_METHOD_MASK
                || raw & 0xFFFF_0000 == PROMISE_RESOLVER_MASK
            {
                return None;
            }
            Some(unsafe { &mut *(raw as *mut JsObject) })
        } else {
            None
        }
    }

    /// Execute a CodeBlock without draining microtasks (used by eval).
    pub fn execute_inner(&mut self, code: &CodeBlock) -> OneResult<JsValue> {
        let stop_depth = self.frames.len();
        let base = self.stack.len();
        self.stack
            .resize(base + code.register_count as usize, JsValue::undefined());

        self.frames.push(CallFrame {
            code: code as *const CodeBlock,
            pc: 0,
            base,
            dest: 0,
            is_constructor: false,
            this_val: JsValue::undefined(),
        });

        self.run_until(Some(stop_depth))
    }

    /// Execute a CodeBlock
    pub fn execute(&mut self, code: &CodeBlock) -> OneResult<JsValue> {
        self.exception_stack.clear();
        self.current_exception = None;

        let result = self.execute_inner(code)?;
        self.drain_microtasks()?;
        Ok(result)
    }

    fn run_until(&mut self, stop_depth: Option<usize>) -> OneResult<JsValue> {
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
                    let equal = if b.is_number() && c.is_number() {
                        b.to_number() == c.to_number()
                    } else {
                        b == c
                    };
                    self.stack[base + instr.a() as usize] = JsValue::from_bool(equal);
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
                Opcode::JumpIfNullish => {
                    let val = self.stack[base + instr.a() as usize];
                    if val.is_nullish() {
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
                    let value = if name == "this" && code.is_strict {
                        JsValue::undefined()
                    } else {
                        self.globals
                            .get(&name)
                            .copied()
                            .unwrap_or(JsValue::undefined())
                    };
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

                    if obj_val.is_string() {
                        if name == "length" {
                            let s = self.value_to_string(obj_val);
                            self.stack[base + dest as usize] =
                                JsValue::from_i32(s.len() as i32);
                            continue;
                        }
                        if let Some(method) = self.get_primitive_method("String", &name) {
                            self.globals.insert("this".to_string(), obj_val);
                            self.stack[base + dest as usize] = method;
                            continue;
                        }
                        self.stack[base + dest as usize] = JsValue::undefined();
                        continue;
                    }

                    if obj_val.is_number() {
                        if let Some(method) = self.get_primitive_method("Number", &name) {
                            self.globals.insert("this".to_string(), obj_val);
                            self.stack[base + dest as usize] = method;
                            continue;
                        }
                        self.stack[base + dest as usize] = JsValue::undefined();
                        continue;
                    }

                    if obj_val.is_boolean() {
                        if let Some(method) = self.get_primitive_method("Boolean", &name) {
                            self.globals.insert("this".to_string(), obj_val);
                            self.stack[base + dest as usize] = method;
                            continue;
                        }
                        self.stack[base + dest as usize] = JsValue::undefined();
                        continue;
                    }

                    if let Some(obj) = self.get_object(obj_val) {
                        let value = obj.get_property(&name).unwrap_or(JsValue::undefined());
                        self.stack[base + dest as usize] = value;
                    } else if let Some(idx) = Self::host_sentinel_idx(obj_val) {
                        if idx < self.host_functions.len() {
                            let base_name = &self.host_functions[idx].0;
                            let method_name = format!("{base_name}.{name}");
                            if let Some(method_idx) = self.find_host_fn(&method_name) {
                                let sentinel = JsValue::from_object_raw(
                                    HOST_SENTINEL_MASK | method_idx as u64,
                                );
                                self.stack[base + dest as usize] = sentinel;
                            } else {
                                self.stack[base + dest as usize] = JsValue::undefined();
                            }
                        } else {
                            self.stack[base + dest as usize] = JsValue::undefined();
                        }
                    } else {
                        self.stack[base + dest as usize] = JsValue::undefined();
                    }
                }
                Opcode::GetElem => {
                    let dest = instr.a();
                    let obj_val = self.stack[base + instr.b() as usize];
                    let key_val = self.stack[base + instr.c() as usize];

                    if let Some(obj) = self.get_object(obj_val) {
                        let key_str = if key_val.is_int32() {
                            key_val.as_i32().unwrap().to_string()
                        } else if key_val.is_float64() {
                            let n = key_val.as_f64().unwrap();
                            if n.fract() == 0.0 && n >= 0.0 {
                                (n as u32).to_string()
                            } else {
                                self.value_to_string(key_val)
                            }
                        } else {
                            self.value_to_string(key_val)
                        };
                        let value = obj.get_property(&key_str).unwrap_or(JsValue::undefined());
                        self.stack[base + dest as usize] = value;
                    } else {
                        self.stack[base + dest as usize] = JsValue::undefined();
                    }
                }
                Opcode::SetElem => {
                    let obj_val = self.stack[base + instr.a() as usize];
                    let key_val = self.stack[base + instr.b() as usize];
                    let value = self.stack[base + instr.c() as usize];

                    let key_str = if key_val.is_int32() {
                        key_val.as_i32().unwrap().to_string()
                    } else {
                        self.value_to_string(key_val)
                    };

                    if let Some(obj) = self.get_object_mut(obj_val) {
                        if let ObjectKind::Array { length } = obj.kind_mut()
                            && let Ok(idx) = key_str.parse::<u32>()
                            && idx >= *length
                        {
                            *length = idx + 1;
                        }
                        obj.set_property(key_str, value);
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
                    let val = self.new_array(len as u32);
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
                Opcode::Spread => {
                    let dest_arr = self.stack[base + instr.a() as usize];
                    let src_val = self.stack[base + instr.b() as usize];
                    self.spread_into_array(dest_arr, src_val);
                }
                Opcode::CreateClosure => {
                    let dest = instr.a();
                    let func_idx = instr.bx() as usize;
                    if func_idx >= code.inner_functions.len() {
                        return Err(OneError::InternalError(format!(
                            "CreateClosure: invalid function index {func_idx}"
                        )));
                    }
                    let inner_code = code.inner_functions[func_idx].clone();
                    let func_obj = FunctionObject {
                        name: inner_code.name.clone(),
                        code: inner_code,
                        param_count: code.inner_functions[func_idx].param_count,
                        upvalues: Vec::new(),
                    };
                    let mut func = JsObject::with_kind(ObjectKind::Function(func_obj));
                    let proto = JsObject::new();
                    let proto_val = self.alloc_object(proto);
                    func.set_property("prototype".to_string(), proto_val);
                    let val = self.alloc_object(func);
                    self.stack[base + dest as usize] = val;
                }
                Opcode::Call => {
                    let dest = instr.a();
                    let func_reg = instr.b();
                    let argc = instr.c() as usize;
                    let func_val = self.stack[base + func_reg as usize];

                    if let Some(idx) = Self::host_sentinel_idx(func_val) {
                        let args: Vec<JsValue> = (0..argc)
                            .map(|i| self.stack[base + func_reg as usize + 1 + i])
                            .collect();
                        let result = self.invoke_host_fn(idx, &args)?;
                        self.stack[base + dest as usize] = result;
                        continue;
                    }

                    if Self::promise_resolver_idx(func_val).is_some() {
                        let args: Vec<JsValue> = (0..argc)
                            .map(|i| self.stack[base + func_reg as usize + 1 + i])
                            .collect();
                        let result = self.call_function(func_val, &args)?;
                        self.stack[base + dest as usize] = result;
                        continue;
                    }

                    if Self::promise_method_idx(func_val).is_some() {
                        let args: Vec<JsValue> = (0..argc)
                            .map(|i| self.stack[base + func_reg as usize + 1 + i])
                            .collect();
                        let result = self.call_function(func_val, &args)?;
                        self.stack[base + dest as usize] = result;
                        continue;
                    }

                    let js_call = self.get_object(func_val).and_then(|obj| {
                        if let ObjectKind::Function(func_obj) = obj.kind() {
                            Some((
                                &func_obj.code as *const CodeBlock,
                                func_obj.param_count,
                                func_obj.code.register_count,
                            ))
                        } else {
                            None
                        }
                    });

                    if let Some((code_ptr, param_count, register_count)) = js_call {
                        let new_base = self.stack.len();
                        self.stack.resize(
                            new_base + register_count as usize,
                            JsValue::undefined(),
                        );

                        for i in 0..argc.min(param_count as usize) {
                            self.stack[new_base + i] =
                                self.stack[base + func_reg as usize + 1 + i];
                        }

                        self.frames.push(CallFrame {
                            code: code_ptr,
                            pc: 0,
                            base: new_base,
                            dest,
                            is_constructor: false,
                            this_val: JsValue::undefined(),
                        });
                        continue;
                    }

                    self.stack[base + dest as usize] = JsValue::undefined();
                }
                Opcode::New => {
                    let dest = instr.a();
                    let ctor_reg = instr.b();
                    let argc = instr.c() as usize;
                    let ctor_val = self.stack[base + ctor_reg as usize];

                    if let Some(idx) = Self::host_sentinel_idx(ctor_val) {
                        let args: Vec<JsValue> = (0..argc)
                            .map(|i| self.stack[base + ctor_reg as usize + 1 + i])
                            .collect();
                        let result = self.invoke_host_fn(idx, &args)?;
                        self.stack[base + dest as usize] = result;
                        continue;
                    }

                    let js_new = self.get_object(ctor_val).and_then(|obj| {
                        if let ObjectKind::Function(func_obj) = obj.kind() {
                            let param_count = func_obj.param_count;
                            let register_count = func_obj.code.register_count;
                            let code_ptr = &func_obj.code as *const CodeBlock;
                            let proto_raw = obj
                                .get_property("prototype")
                                .and_then(|v| v.as_object_raw());
                            Some((code_ptr, param_count, register_count, proto_raw))
                        } else {
                            None
                        }
                    });

                    if let Some((code_ptr, param_count, register_count, proto_raw)) = js_new {
                        let mut instance = JsObject::new();
                        if let Some(raw) = proto_raw {
                            instance.set_prototype(Some(raw as *mut JsObject));
                        }
                        let instance_val = self.alloc_object(instance);
                        self.globals.insert("this".to_string(), instance_val);

                        let new_base = self.stack.len();
                        self.stack.resize(
                            new_base + register_count as usize,
                            JsValue::undefined(),
                        );

                        for i in 0..argc.min(param_count as usize) {
                            self.stack[new_base + i] =
                                self.stack[base + ctor_reg as usize + 1 + i];
                        }

                        self.frames.push(CallFrame {
                            code: code_ptr,
                            pc: 0,
                            base: new_base,
                            dest,
                            is_constructor: true,
                            this_val: instance_val,
                        });
                        continue;
                    }
                    self.stack[base + dest as usize] = JsValue::undefined();
                }
                Opcode::Return => {
                    let val = self.stack[base + instr.a() as usize];
                    let frame = self.frames.pop().unwrap();
                    self.stack.truncate(frame.base);
                    let return_val = if frame.is_constructor && !val.is_object() {
                        frame.this_val
                    } else {
                        val
                    };
                    if self.frames.is_empty() {
                        return Ok(return_val);
                    }
                    if stop_depth == Some(self.frames.len()) {
                        return Ok(return_val);
                    }
                    let caller_base = self.frames.last().unwrap().base;
                    self.stack[caller_base + frame.dest as usize] = return_val;
                }
                Opcode::ReturnUndef => {
                    let frame = self.frames.pop().unwrap();
                    self.stack.truncate(frame.base);
                    let return_val = if frame.is_constructor {
                        frame.this_val
                    } else {
                        JsValue::undefined()
                    };
                    if self.frames.is_empty() {
                        return Ok(return_val);
                    }
                    if stop_depth == Some(self.frames.len()) {
                        return Ok(return_val);
                    }
                    let caller_base = self.frames.last().unwrap().base;
                    self.stack[caller_base + frame.dest as usize] = return_val;
                }
                Opcode::TryStart => {
                    let catch_pc =
                        (self.frames[frame_idx].pc as i32 + instr.sbx() as i32) as usize;
                    self.exception_stack.push(ExceptionHandlerFrame {
                        catch_pc,
                        frame_idx,
                    });
                }
                Opcode::TryEnd => {
                    self.exception_stack.pop();
                }
                Opcode::Throw => {
                    let val = self.stack[base + instr.a() as usize];
                    self.throw_exception(val)?;
                    continue;
                }
                Opcode::CatchBind => {
                    let dest = instr.a();
                    let val = self
                        .current_exception
                        .take()
                        .unwrap_or(JsValue::undefined());
                    self.stack[base + dest as usize] = val;
                }
                Opcode::TypeOf => {
                    let dest = instr.a();
                    let val = self.stack[base + instr.b() as usize];
                    let type_str = val.type_of();
                    self.stack[base + dest as usize] = self.alloc_string(type_str.to_string());
                }
                _ => {}
            }
        }
    }

    fn throw_exception(&mut self, val: JsValue) -> OneResult<()> {
        self.current_exception = Some(val);

        if let Some(handler) = self.exception_stack.pop() {
            while self.frames.len() > handler.frame_idx + 1 {
                let popped_frame_idx = self.frames.len() - 1;
                let frame = self.frames.pop().unwrap();
                self.stack.truncate(frame.base);
                self.exception_stack
                    .retain(|h| h.frame_idx != popped_frame_idx);
            }

            self.frames[handler.frame_idx].pc = handler.catch_pc;
            Ok(())
        } else {
            let message = self.value_to_string(val);
            Err(OneError::JsException(one_core::JsException {
                name: "Error".to_string(),
                message,
                stack_trace: Vec::new(),
            }))
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

    pub fn alloc_string(&mut self, s: String) -> JsValue {
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

    fn spread_into_array(&mut self, dest_arr: JsValue, src_val: JsValue) {
        let elements = if let Some(src_obj) = self.get_object(src_val) {
            if let ObjectKind::Array { length } = src_obj.kind() {
                (0..*length)
                    .map(|i| {
                        src_obj
                            .get_property(&i.to_string())
                            .unwrap_or(JsValue::undefined())
                    })
                    .collect::<Vec<_>>()
            } else {
                return;
            }
        } else {
            return;
        };

        let Some(dest_obj) = self.get_object_mut(dest_arr) else {
            return;
        };
        let start = match dest_obj.kind() {
            ObjectKind::Array { length } => *length,
            _ => 0,
        };

        for (offset, val) in elements.iter().enumerate() {
            dest_obj.set_property((start + offset as u32).to_string(), *val);
        }
        if let ObjectKind::Array { length } = dest_obj.kind_mut() {
            *length = start + elements.len() as u32;
        }
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

    pub fn snapshot_globals(&self) -> HashMap<String, JsValue> {
        self.globals.clone()
    }

    pub fn restore_globals(&mut self, globals: HashMap<String, JsValue>) {
        self.globals = globals;
    }

    pub fn create_object_from_pairs(&mut self, pairs: &[(String, JsValue)]) -> JsValue {
        let mut obj = JsObject::new();
        for (key, val) in pairs {
            obj.set_property(key.clone(), *val);
        }
        self.alloc_object(obj)
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
