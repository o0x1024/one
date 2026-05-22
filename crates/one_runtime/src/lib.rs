pub mod array;
pub mod boolean;
pub mod collections;
pub mod console;
pub mod date;
pub mod error;
pub mod globals;
pub mod json;
pub mod math;
pub mod number;
pub mod object;
pub mod preset;
pub mod promise;
pub mod regexp;
pub mod string;
pub mod symbol;
pub mod timers;

pub use preset::{BuiltinModule, Preset, install_preset};

use one_vm::Vm;

/// Install all built-in runtime APIs
pub fn install_builtins(vm: &mut Vm) {
    console::install_console(vm);
    promise::install_promise(vm);
    timers::install_timers(vm);
    globals::install_globals(vm);
    object::install_object(vm);
    array::install_array(vm);
    collections::install_collections(vm);
    string::install_string(vm);
    number::install_number(vm);
    boolean::install_boolean(vm);
    math::install_math(vm);
    json::install_json(vm);
    error::install_error(vm);
    date::install_date(vm);
    symbol::install_symbol(vm);
    regexp::install_regexp(vm);
}
