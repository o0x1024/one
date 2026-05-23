use std::marker::PhantomData;

use one_core::{JsValue, OneResult};
use one_vm::Vm;

use crate::engine::Engine;
use crate::extension::Extension;
use crate::limits::RuntimeLimits;
use crate::module_resolver::{
    FileModuleResolver, ModuleResolver, ModuleResolverChain, StaticModuleResolver,
    UrlModuleResolver,
};
use crate::preset::Preset;

type HostFn = Box<dyn Fn(&mut Vm, &[JsValue]) -> OneResult<JsValue>>;

pub struct EngineBuilder<T: 'static = ()> {
    preset: Preset,
    host_functions: Vec<(String, HostFn)>,
    globals: Vec<(String, JsValue)>,
    modules: Vec<(String, String)>,
    extensions: Vec<Box<dyn Extension>>,
    module_resolver: Option<Box<dyn ModuleResolver>>,
    limits: Option<RuntimeLimits>,
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
            extensions: Vec::new(),
            module_resolver: None,
            limits: None,
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

    pub fn extension(mut self, ext: impl Extension) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    pub fn module_resolver(mut self, resolver: impl ModuleResolver) -> Self {
        self.module_resolver = Some(Box::new(resolver));
        self
    }

    pub fn limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    pub fn build_with_store(self, store: T) -> Engine<T> {
        let mut vm = Vm::new();
        one_runtime::install_preset(&mut vm, &self.preset);

        // Register user-level host functions.
        for (name, func) in self.host_functions {
            vm.register_host_fn(&name, move |vm, args| func(vm, args));
        }

        // Set user-level globals.
        for (name, value) in self.globals {
            vm.set_global(&name, value);
        }

        // Process extensions: host functions → globals → bootstrap JS.
        let mut bootstrap_scripts: Vec<&str> = Vec::new();
        let extensions: Vec<Box<dyn Extension>> = self.extensions;
        for ext in &extensions {
            for desc in ext.host_functions() {
                vm.register_host_fn(&desc.name, move |vm, args| (desc.func)(vm, args));
            }
            for (name, value) in ext.globals() {
                vm.set_global(&name, value);
            }
            if let Some(js) = ext.bootstrap_js() {
                bootstrap_scripts.push(js);
            }
        }

        if let Some(threshold) = self.gc_threshold {
            vm.set_gc_threshold(threshold);
        }

        // Apply RuntimeLimits: max_operations overrides fuel_limit.
        let effective_fuel = if let Some(ref limits) = self.limits {
            limits.max_operations
        } else {
            self.fuel_limit.or(match self.preset {
                Preset::Sandbox => Some(crate::preset::SANDBOX_DEFAULT_FUEL),
                _ => None,
            })
        };
        if let Some(limit) = effective_fuel {
            vm.set_fuel(limit);
        }

        if let Some(ref limits) = self.limits {
            if let Some(max_depth) = limits.max_call_depth {
                vm.set_max_call_depth(max_depth);
            }
        }

        // Initialize extension state in TypeMap.
        let mut type_map = crate::type_map::TypeMap::new();
        for ext in &extensions {
            ext.init_state(&mut type_map);
        }

        // Execute bootstrap JS from extensions.
        for script in bootstrap_scripts {
            let program = one_parser::parser::Parser::parse(script)
                .expect("Extension bootstrap JS parse error");
            let code = one_compiler::Compiler::compile(&program);
            vm.execute(&code).expect("Extension bootstrap JS execution error");
        }

        // Build module resolver: use custom if provided, else build a default chain.
        let module_resolver: Box<dyn ModuleResolver> = if let Some(resolver) = self.module_resolver
        {
            resolver
        } else {
            let mut static_resolver = StaticModuleResolver::new();
            for (name, source) in &self.modules {
                static_resolver.register(name, source);
            }
            let chain = ModuleResolverChain::new()
                .push(static_resolver)
                .push(FileModuleResolver::from_cwd())
                .push(UrlModuleResolver::with_default_cache());
            Box::new(chain)
        };

        let baseline_globals = vm.snapshot_globals();

        Engine::from_parts(
            vm,
            store,
            type_map,
            module_resolver,
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

    #[test]
    fn builder_with_extension() {
        use crate::extension::{host_fn, Extension, HostFnDescriptor};

        struct MathExt;
        impl Extension for MathExt {
            fn name(&self) -> &str {
                "math_ext"
            }
            fn host_functions(&self) -> Vec<HostFnDescriptor> {
                vec![host_fn("math_ext.double", |_vm, args| {
                    let n = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                    Ok(JsValue::from_f64(n * 2.0))
                })]
            }
            fn globals(&self) -> Vec<(String, JsValue)> {
                vec![("MATH_EXT_VERSION".to_string(), JsValue::from_i32(1))]
            }
            fn bootstrap_js(&self) -> Option<&str> {
                None
            }
        }

        let mut engine = EngineBuilder::<()>::new()
            .extension(MathExt)
            .build();
        let result = engine.eval("return math_ext.double(21);").unwrap();
        assert!(result.to_number() == 42.0);
        let ver = engine.eval("return MATH_EXT_VERSION;").unwrap();
        assert!(ver.to_number() == 1.0);
    }

    #[test]
    fn builder_extension_with_bootstrap() {
        use crate::extension::{host_fn, Extension, HostFnDescriptor};

        struct GreetExt;
        impl Extension for GreetExt {
            fn name(&self) -> &str {
                "greet"
            }
            fn host_functions(&self) -> Vec<HostFnDescriptor> {
                vec![host_fn("__greet_raw", |vm, args| {
                    let name = args
                        .first()
                        .map(|v| vm.value_to_string(*v))
                        .unwrap_or_default();
                    Ok(vm.alloc_string(format!("Hello, {name}!")))
                })]
            }
            fn bootstrap_js(&self) -> Option<&str> {
                Some("function greet(name) { return __greet_raw(name); }")
            }
        }

        let mut engine = EngineBuilder::<()>::new()
            .extension(GreetExt)
            .build();
        let result = engine.eval(r#"return greet("World");"#).unwrap();
        let s = engine.vm().value_to_string(result);
        assert_eq!(s, "Hello, World!");
    }

    #[test]
    fn builder_with_custom_module_resolver() {
        use crate::module_resolver::StaticModuleResolver;

        let mut resolver = StaticModuleResolver::new();
        resolver.register("math", "export let TAU = 6.28;");

        let mut engine = EngineBuilder::<()>::new()
            .module_resolver(resolver)
            .build();

        let result = engine
            .eval_module(r#"import { TAU } from "math"; return TAU;"#, "<test>")
            .unwrap();
        assert!((result.to_number() - 6.28).abs() < 0.01);
    }

    #[test]
    fn builder_with_limits_call_depth() {
        use crate::limits::RuntimeLimits;
        use one_core::OneError;

        let mut engine = EngineBuilder::<()>::new()
            .limits(RuntimeLimits {
                max_call_depth: Some(5),
                ..Default::default()
            })
            .build();

        let result = engine.eval(
            r#"
            function recurse(n) {
                if (n <= 0) return 0;
                return recurse(n - 1);
            }
            return recurse(100);
        "#,
        );

        assert!(matches!(result, Err(OneError::StackOverflow { .. })));
    }

    #[test]
    fn builder_with_limits_operations() {
        use crate::limits::RuntimeLimits;
        use one_core::OneError;

        let mut engine = EngineBuilder::<()>::new()
            .limits(RuntimeLimits {
                max_operations: Some(100),
                ..Default::default()
            })
            .build();

        let result = engine.eval("let x = 0; while (true) { x = x + 1; } return x;");
        assert!(matches!(result, Err(OneError::OutOfFuel { .. })));
    }
}
