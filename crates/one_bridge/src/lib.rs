//! Adapter layer between One Engine and Sentinel's plugin system.

pub mod extensions;
pub mod ops;
pub mod runtime;
pub mod sentinel_api;

pub use extensions::{
    AstExtension, DictionaryExtension, FetchExtension, FsExtension, MonitorExtension,
    NetworkExtension, SentinelCoreExtension, TlsExtension,
};
pub use runtime::{Finding, PluginRuntime, PluginState};
