use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use one_core::{OneError, OneResult};

pub trait ModuleResolver: Send + 'static {
    fn resolve(&self, specifier: &str, referrer: Option<&str>) -> OneResult<String>;
    fn load(&self, resolved_path: &str) -> OneResult<String>;
    fn as_any_mut(&mut self) -> &mut dyn Any {
        panic!("as_any_mut not implemented for this ModuleResolver")
    }
}

// ---------------------------------------------------------------------------
// StaticModuleResolver — in-memory pre-registered modules
// ---------------------------------------------------------------------------

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
            Err(OneError::InternalError(format!(
                "Module not found: {specifier}"
            )))
        }
    }

    fn load(&self, resolved_path: &str) -> OneResult<String> {
        self.modules.get(resolved_path).cloned().ok_or_else(|| {
            OneError::InternalError(format!("Module not found: {resolved_path}"))
        })
    }
}

// ---------------------------------------------------------------------------
// FileModuleResolver — load .js/.ts files from disk
// ---------------------------------------------------------------------------

pub struct FileModuleResolver {
    base_dir: PathBuf,
}

impl FileModuleResolver {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn from_cwd() -> Self {
        Self {
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    fn can_handle(&self, specifier: &str) -> bool {
        specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier.starts_with('/')
            || specifier.ends_with(".js")
            || specifier.ends_with(".mjs")
            || specifier.ends_with(".ts")
    }

    fn resolve_path(&self, specifier: &str, referrer: Option<&str>) -> PathBuf {
        let base = if let Some(referrer) = referrer {
            let ref_path = Path::new(referrer);
            if ref_path.is_absolute() {
                ref_path
                    .parent()
                    .unwrap_or(Path::new("/"))
                    .to_path_buf()
            } else {
                self.base_dir.join(ref_path).parent().map(|p| p.to_path_buf())
                    .unwrap_or_else(|| self.base_dir.clone())
            }
        } else {
            self.base_dir.clone()
        };

        let candidate = base.join(specifier);
        if candidate.is_absolute() {
            candidate
        } else {
            self.base_dir.join(candidate)
        }
    }
}

impl ModuleResolver for FileModuleResolver {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn resolve(&self, specifier: &str, referrer: Option<&str>) -> OneResult<String> {
        if !self.can_handle(specifier) {
            return Err(OneError::InternalError(format!(
                "Module not found: {specifier}"
            )));
        }

        let path = self.resolve_path(specifier, referrer);

        let extensions = ["", ".js", ".mjs", ".ts"];
        for ext in &extensions {
            let candidate = if ext.is_empty() {
                path.clone()
            } else {
                path.with_extension(ext.trim_start_matches('.'))
            };
            if candidate.is_file() {
                return candidate
                    .canonicalize()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| OneError::InternalError(format!("resolve error: {e}")));
            }
        }

        // Try index.js inside directory
        if path.is_dir() {
            let index = path.join("index.js");
            if index.is_file() {
                return index
                    .canonicalize()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| OneError::InternalError(format!("resolve error: {e}")));
            }
        }

        Err(OneError::InternalError(format!(
            "Module not found: {specifier} (searched from {})",
            path.display()
        )))
    }

    fn load(&self, resolved_path: &str) -> OneResult<String> {
        std::fs::read_to_string(resolved_path).map_err(|e| {
            OneError::InternalError(format!("Failed to read module {resolved_path}: {e}"))
        })
    }
}

// ---------------------------------------------------------------------------
// UrlModuleResolver — fetch modules from HTTP(S) URLs with disk cache
// ---------------------------------------------------------------------------

pub struct UrlModuleResolver {
    cache_dir: PathBuf,
}

impl UrlModuleResolver {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        let dir = cache_dir.into();
        let _ = std::fs::create_dir_all(&dir);
        Self { cache_dir: dir }
    }

    pub fn with_default_cache() -> Self {
        let dir = dirs_fallback().join("one").join("module_cache");
        Self::new(dir)
    }

    fn can_handle(&self, specifier: &str) -> bool {
        specifier.starts_with("http://") || specifier.starts_with("https://")
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        let safe_name: String = url
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
            .collect();
        self.cache_dir.join(safe_name)
    }
}

impl ModuleResolver for UrlModuleResolver {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn resolve(&self, specifier: &str, _referrer: Option<&str>) -> OneResult<String> {
        if !self.can_handle(specifier) {
            return Err(OneError::InternalError(format!(
                "Module not found: {specifier}"
            )));
        }
        Ok(specifier.to_string())
    }

    fn load(&self, resolved_path: &str) -> OneResult<String> {
        let cached = self.cache_path(resolved_path);
        if cached.is_file() {
            return std::fs::read_to_string(&cached).map_err(|e| {
                OneError::InternalError(format!("Failed to read cached module: {e}"))
            });
        }

        #[cfg(feature = "net")]
        {
            let body = ureq::get(resolved_path)
                .call()
                .map_err(|e| OneError::InternalError(format!("Failed to fetch {resolved_path}: {e}")))?
                .into_body()
                .read_to_string()
                .map_err(|e| OneError::InternalError(format!("Failed to read response body: {e}")))?;

            if let Some(parent) = cached.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cached, &body);

            Ok(body)
        }

        #[cfg(not(feature = "net"))]
        {
            Err(OneError::InternalError(format!(
                "URL module loading requires the 'net' feature: {resolved_path}"
            )))
        }
    }
}

fn dirs_fallback() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        PathBuf::from(home).join(".cache")
    } else {
        PathBuf::from("/tmp")
    }
}

// ---------------------------------------------------------------------------
// ModuleResolverChain — composable resolver chain (Rhai-style)
// ---------------------------------------------------------------------------

pub struct ModuleResolverChain {
    resolvers: Vec<Box<dyn ModuleResolver>>,
}

impl ModuleResolverChain {
    pub fn new() -> Self {
        Self {
            resolvers: Vec::new(),
        }
    }

    pub fn push(mut self, resolver: impl ModuleResolver) -> Self {
        self.resolvers.push(Box::new(resolver));
        self
    }
}

impl ModuleResolverChain {
    pub fn register_static_module(&mut self, name: &str, source: &str) -> bool {
        for resolver in &mut self.resolvers {
            if let Some(static_resolver) = resolver.as_any_mut().downcast_mut::<StaticModuleResolver>()
            {
                static_resolver.register(name, source);
                return true;
            }
        }
        false
    }
}

impl Default for ModuleResolverChain {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleResolver for ModuleResolverChain {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn resolve(&self, specifier: &str, referrer: Option<&str>) -> OneResult<String> {
        for resolver in &self.resolvers {
            if let Ok(resolved) = resolver.resolve(specifier, referrer) {
                return Ok(resolved);
            }
        }
        Err(OneError::InternalError(format!(
            "Module not found in any resolver: {specifier}"
        )))
    }

    fn load(&self, resolved_path: &str) -> OneResult<String> {
        for resolver in &self.resolvers {
            if let Ok(source) = resolver.load(resolved_path) {
                return Ok(source);
            }
        }
        Err(OneError::InternalError(format!(
            "Module not loadable from any resolver: {resolved_path}"
        )))
    }
}
