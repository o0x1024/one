use std::marker::PhantomData;

use one_core::{JsValue, OneResult};
use one_vm::Vm;

use crate::engine::Engine;
use crate::preset::Preset;

type HostFn = Box<dyn Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue>>;

pub struct EngineBuilder<T: 'static = ()> {
    preset: Preset,
    host_functions: Vec<(String, HostFn)>,
    globals: Vec<(String, JsValue)>,
    modules: Vec<(String, String)>,
    fuel_limit: Option<u64>,
    gc_threshold: Option<usize>,
    _marker: PhantomData<T>,
}

impl<T: 'static> EngineBuilder<T> {
    pub fn new() -> Self {
        Self {
            preset: Preset::default(),
            host_functions: Vec::new(),
            globals: Vec::new(),
            modules: Vec::new(),
            fuel_limit: None,
            gc_threshold: None,
            _marker: PhantomData,
        }
    }

    pub fn preset(mut self, preset: Preset) -> Self {
        self.preset = preset;
        self
    }

    pub fn with_host_fn<F>(mut self, name: &str, func: F) -> Self
    where
        F: Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue> + 'static,
    {
        self.host_functions
            .push((name.to_string(), Box::new(func)));
        self
    }

    pub fn with_global(mut self, name: &str, value: JsValue) -> Self {
        self.globals.push((name.to_string(), value));
        self
    }

    pub fn with_module(mut self, name: &str, source: &str) -> Self {
        self.modules
            .push((name.to_string(), source.to_string()));
        self
    }

    pub fn fuel_limit(mut self, limit: u64) -> Self {
        self.fuel_limit = Some(limit);
        self
    }

    pub fn gc_threshold(mut self, threshold: usize) -> Self {
        self.gc_threshold = Some(threshold);
        self
    }

    pub fn build_with_store(self, store: T) -> Engine<T> {
        let mut vm = Vm::new();
        one_runtime::install_preset(&mut vm, &self.preset);

        for (name, func) in self.host_functions {
            vm.register_host_fn(&name, move |vm, args| func(vm, args));
        }

        for (name, value) in self.globals {
            vm.set_global(&name, value);
        }

        if let Some(threshold) = self.gc_threshold {
            vm.set_gc_threshold(threshold);
        }

        let fuel_limit = self.fuel_limit.or(match self.preset {
            Preset::Sandbox => Some(crate::preset::SANDBOX_DEFAULT_FUEL),
            _ => None,
        });
        if let Some(limit) = fuel_limit {
            vm.set_fuel(limit);
        }

        let baseline_globals = vm.snapshot_globals();

        Engine::from_parts(
            vm,
            store,
            self.modules.into_iter().collect(),
            Default::default(),
            baseline_globals,
        )
    }
}

impl EngineBuilder<()> {
    pub fn build(self) -> Engine<()> {
        self.build_with_store(())
    }
}

impl<T: 'static> Default for EngineBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::BuiltinModule;

    #[test]
    fn builder_default() {
        let engine = EngineBuilder::<()>::new().build();
        assert!(engine.vm().get_global("console").is_object());
    }

    #[test]
    fn builder_minimal() {
        let engine = EngineBuilder::<()>::new()
            .preset(Preset::Minimal)
            .build();
        assert!(engine.vm().get_global("console").is_undefined());
    }

    #[test]
    fn builder_with_store() {
        struct MyState {
            counter: u32,
        }
        let engine = EngineBuilder::<MyState>::new()
            .build_with_store(MyState { counter: 0 });
        assert_eq!(engine.store().counter, 0);
    }

    #[test]
    fn builder_with_global() {
        let mut engine = EngineBuilder::<()>::new()
            .with_global("MY_CONST", JsValue::from_i32(42))
            .build();
        let result = engine.eval("return MY_CONST;").unwrap();
        assert!(result.to_number() == 42.0);
    }

    #[test]
    fn builder_with_module() {
        let mut engine = EngineBuilder::<()>::new()
            .with_module("utils", "export let PI = 3.14;")
            .build();
        let result = engine
            .eval_module(r#"import { PI } from "utils"; return PI;"#, "<test>")
            .unwrap();
        assert!((result.to_number() - 3.14).abs() < 0.01);
    }

    #[test]
    fn builder_custom_preset() {
        let engine = EngineBuilder::<()>::new()
            .preset(Preset::Custom(vec![
                BuiltinModule::Console,
                BuiltinModule::Math,
            ]))
            .build();
        assert!(engine.vm().get_global("console").is_object());
        assert!(engine.vm().get_global("Math").is_object());
    }

    #[test]
    fn store_mutation() {
        struct Counter {
            value: i32,
        }
        let mut engine = EngineBuilder::<Counter>::new()
            .build_with_store(Counter { value: 0 });
        engine.store_mut().value += 1;
        assert_eq!(engine.store().value, 1);
    }
}
