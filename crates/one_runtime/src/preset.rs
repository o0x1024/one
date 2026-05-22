use one_vm::Vm;

use crate::{
    array, collections, console, date, error, globals, json, math, number, object, promise,
    regexp, string, symbol, timers,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Full,
    Minimal,
    Sandbox,
    Custom(Vec<BuiltinModule>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinModule {
    Console,
    Math,
    JSON,
    Promise,
    Timers,
    Object,
    Array,
    String,
    Number,
    Date,
    RegExp,
    Map,
    Set,
    Symbol,
    Error,
    Globals,
}

pub fn install_preset(vm: &mut Vm, preset: &Preset) {
    match preset {
        Preset::Full => crate::install_builtins(vm),
        Preset::Minimal => {
            object::install_object(vm);
            error::install_error(vm);
        }
        Preset::Sandbox => {
            crate::install_builtins(vm);
        }
        Preset::Custom(modules) => {
            let mut collections_installed = false;
            for module in modules {
                match module {
                    BuiltinModule::Console => console::install_console(vm),
                    BuiltinModule::Math => math::install_math(vm),
                    BuiltinModule::JSON => json::install_json(vm),
                    BuiltinModule::Promise => promise::install_promise(vm),
                    BuiltinModule::Timers => timers::install_timers(vm),
                    BuiltinModule::Object => object::install_object(vm),
                    BuiltinModule::Array => array::install_array(vm),
                    BuiltinModule::String => string::install_string(vm),
                    BuiltinModule::Number => number::install_number(vm),
                    BuiltinModule::Date => date::install_date(vm),
                    BuiltinModule::RegExp => regexp::install_regexp(vm),
                    BuiltinModule::Map | BuiltinModule::Set => {
                        if !collections_installed {
                            collections::install_collections(vm);
                            collections_installed = true;
                        }
                    }
                    BuiltinModule::Symbol => symbol::install_symbol(vm),
                    BuiltinModule::Error => error::install_error(vm),
                    BuiltinModule::Globals => globals::install_globals(vm),
                }
            }
        }
    }
}
