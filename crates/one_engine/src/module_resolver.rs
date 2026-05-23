use std::any::Any;
use std::collections::HashMap;

use one_core::OneResult;

pub trait ModuleResolver: Send + 'static {
    fn resolve(&self, specifier: &str, referrer: Option<&str>) -> OneResult<String>;
    fn load(&self, resolved_path: &str) -> OneResult<String>;
    fn as_any_mut(&mut self) -> &mut dyn Any {
        panic!("as_any_mut not implemented for this ModuleResolver")
    }
}

/// Default resolver that looks up modules from a pre-registered in-memory map.
pub struct StaticModuleResolver {
    modules: HashMap<String, String>,
}

impl StaticModuleResolver {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    pub fn register(&mut self, specifier: &str, source: &str) {
        self.modules
            .insert(specifier.to_string(), source.to_string());
    }
}

impl Default for StaticModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleResolver for StaticModuleResolver {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn resolve(&self, specifier: &str, _referrer: Option<&str>) -> OneResult<String> {
        if self.modules.contains_key(specifier) {
            Ok(specifier.to_string())
        } else {
            Err(one_core::OneError::InternalError(format!(
                "Module not found: {specifier}"
            )))
        }
    }

    fn load(&self, resolved_path: &str) -> OneResult<String> {
        self.modules.get(resolved_path).cloned().ok_or_else(|| {
            one_core::OneError::InternalError(format!(
                "Module not found: {resolved_path}"
            ))
        })
    }
}
