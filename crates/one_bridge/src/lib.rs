//! Adapter layer between One Engine and Sentinel's plugin system.

pub mod ops;
pub mod runtime;
pub mod sentinel_api;

pub use runtime::{Finding, PluginRuntime, PluginState};
